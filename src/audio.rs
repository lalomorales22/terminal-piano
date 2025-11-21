use anyhow::Result;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::piano::Note;

pub struct AudioEngine {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sinks: Arc<Mutex<HashMap<u8, Sink>>>,
    samples: HashMap<u8, Vec<u8>>,
    volume: f32,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) = match OutputStream::try_default() {
            Ok((s, h)) => (s, h),
            Err(e) => {
                eprintln!("Failed to create default audio output stream: {}", e);
                eprintln!("Trying alternative audio initialization...");
                
                // Try to create with specific device/format
                match rodio::OutputStream::try_default() {
                    Ok((s, h)) => (s, h),
                    Err(e2) => {
                        eprintln!("Alternative audio initialization also failed: {}", e2);
                        return Err(anyhow::anyhow!("Could not initialize audio: {} (fallback: {})", e, e2));
                    }
                }
            }
        };
        
        let mut engine = Self {
            _stream: stream,
            stream_handle,
            sinks: Arc::new(Mutex::new(HashMap::new())),
            samples: HashMap::new(),
            volume: 0.7,
        };
        
        engine.load_samples()?;
        Ok(engine)
    }
    
    fn load_samples(&mut self) -> Result<()> {
        for midi_note in 21..109 {
            let sample = self.generate_sine_wave(Note::new(midi_note).frequency(), 1.0);
            self.samples.insert(midi_note, sample);
        }
        Ok(())
    }
    
    fn generate_sine_wave(&self, frequency: f32, duration: f32) -> Vec<u8> {
        let sample_rate = 44100;
        let samples = (sample_rate as f32 * duration) as usize;
        let mut data = Vec::with_capacity(samples * 2);
        
        for i in 0..samples {
            let t = i as f32 / sample_rate as f32;
            
            let envelope = if t < 0.1 {
                t / 0.1
            } else if t > duration - 0.3 {
                (duration - t) / 0.3
            } else {
                1.0
            };
            
            let value = (2.0 * std::f32::consts::PI * frequency * t).sin() * envelope * 0.3;
            let sample = (value * i16::MAX as f32) as i16;
            
            data.push((sample & 0xFF) as u8);
            data.push(((sample >> 8) & 0xFF) as u8);
        }
        
        data
    }
    
    pub fn play_note(&self, midi_note: u8) -> Result<()> {
        // Stop any existing note on this key first
        self.stop_note(midi_note);
        
        if let Some(sample_data) = self.samples.get(&midi_note) {
            let cursor = std::io::Cursor::new(sample_data.clone());
            let source = PcmSource::new(cursor, 44100, 1)?;
            
            let sink = Sink::try_new(&self.stream_handle)?;
            sink.set_volume(self.volume);
            sink.append(source);
            sink.play();
            
            {
                let mut sinks = self.sinks.lock().unwrap();
                sinks.insert(midi_note, sink);
            }
        }
        
        Ok(())
    }
    
    pub fn stop_note(&self, midi_note: u8) {
        let mut sinks = self.sinks.lock().unwrap();
        if let Some(sink) = sinks.remove(&midi_note) {
            sink.stop();
        }
    }
    
    #[allow(dead_code)]
    pub fn stop_all_notes(&self) {
        let mut sinks = self.sinks.lock().unwrap();
        for (_, sink) in sinks.drain() {
            sink.stop();
        }
    }
    
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        
        let sinks = self.sinks.lock().unwrap();
        for (_, sink) in sinks.iter() {
            sink.set_volume(self.volume);
        }
    }
    
    #[allow(dead_code)]
    pub fn get_volume(&self) -> f32 {
        self.volume
    }
    
    pub fn cleanup_finished_notes(&self) {
        let mut sinks = self.sinks.lock().unwrap();
        sinks.retain(|_, sink| !sink.empty());
    }
}

struct PcmSource {
    data: std::io::Cursor<Vec<u8>>,
    sample_rate: u32,
    channels: u16,
}

impl PcmSource {
    fn new(data: std::io::Cursor<Vec<u8>>, sample_rate: u32, channels: u16) -> Result<Self> {
        Ok(Self {
            data,
            sample_rate,
            channels,
        })
    }
}

impl Source for PcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        let total_samples = self.data.get_ref().len() / 2 / self.channels as usize;
        Some(Duration::from_secs_f32(total_samples as f32 / self.sample_rate as f32))
    }
}

impl Iterator for PcmSource {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        let mut bytes = [0u8; 2];
        if std::io::Read::read_exact(&mut self.data, &mut bytes).is_ok() {
            Some(i16::from_le_bytes(bytes))
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Recording {
    pub events: Vec<RecordingEvent>,
    pub duration: Duration,
    pub start_time: std::time::Instant,
}

#[derive(Debug, Clone)]
pub struct RecordingEvent {
    pub timestamp: Duration,
    pub event_type: RecordingEventType,
}

#[derive(Debug, Clone)]
pub enum RecordingEventType {
    NoteOn { midi_note: u8, velocity: u8 },
    NoteOff { midi_note: u8 },
    SustainPedal { pressed: bool },
}

impl Recording {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            duration: Duration::default(),
            start_time: std::time::Instant::now(),
        }
    }
    
    pub fn add_event(&mut self, event_type: RecordingEventType) {
        let timestamp = self.start_time.elapsed();
        self.events.push(RecordingEvent {
            timestamp,
            event_type,
        });
        self.duration = timestamp;
    }
    
    pub fn finish(&mut self) {
        self.duration = self.start_time.elapsed();
    }
    
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<()> {
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
    
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let recording: Recording = serde_json::from_str(&data)?;
        Ok(recording)
    }
}

impl serde::Serialize for Recording {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Recording", 2)?;
        state.serialize_field("events", &self.events)?;
        state.serialize_field("duration_ms", &self.duration.as_millis())?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for Recording {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct RecordingVisitor;

        impl<'de> Visitor<'de> for RecordingVisitor {
            type Value = Recording;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Recording")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Recording, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut events = None;
                let mut duration_ms = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "events" => {
                            if events.is_some() {
                                return Err(de::Error::duplicate_field("events"));
                            }
                            events = Some(map.next_value()?);
                        }
                        "duration_ms" => {
                            if duration_ms.is_some() {
                                return Err(de::Error::duplicate_field("duration_ms"));
                            }
                            duration_ms = Some(map.next_value()?);
                        }
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }
                let events = events.ok_or_else(|| de::Error::missing_field("events"))?;
                let duration_ms: u64 = duration_ms.ok_or_else(|| de::Error::missing_field("duration_ms"))?;
                Ok(Recording {
                    events,
                    duration: Duration::from_millis(duration_ms),
                    start_time: std::time::Instant::now(),
                })
            }
        }

        deserializer.deserialize_struct("Recording", &["events", "duration_ms"], RecordingVisitor)
    }
}

impl serde::Serialize for RecordingEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("RecordingEvent", 2)?;
        state.serialize_field("timestamp_ms", &self.timestamp.as_millis())?;
        state.serialize_field("event_type", &self.event_type)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for RecordingEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct RecordingEventVisitor;

        impl<'de> Visitor<'de> for RecordingEventVisitor {
            type Value = RecordingEvent;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct RecordingEvent")
            }

            fn visit_map<V>(self, mut map: V) -> Result<RecordingEvent, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut timestamp_ms = None;
                let mut event_type = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "timestamp_ms" => {
                            if timestamp_ms.is_some() {
                                return Err(de::Error::duplicate_field("timestamp_ms"));
                            }
                            timestamp_ms = Some(map.next_value()?);
                        }
                        "event_type" => {
                            if event_type.is_some() {
                                return Err(de::Error::duplicate_field("event_type"));
                            }
                            event_type = Some(map.next_value()?);
                        }
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }
                let timestamp_ms: u64 = timestamp_ms.ok_or_else(|| de::Error::missing_field("timestamp_ms"))?;
                let event_type = event_type.ok_or_else(|| de::Error::missing_field("event_type"))?;
                Ok(RecordingEvent {
                    timestamp: Duration::from_millis(timestamp_ms),
                    event_type,
                })
            }
        }

        deserializer.deserialize_struct("RecordingEvent", &["timestamp_ms", "event_type"], RecordingEventVisitor)
    }
}

impl serde::Serialize for RecordingEventType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        match self {
            RecordingEventType::NoteOn { midi_note, velocity } => {
                let mut state = serializer.serialize_struct("NoteOn", 3)?;
                state.serialize_field("type", "NoteOn")?;
                state.serialize_field("midi_note", midi_note)?;
                state.serialize_field("velocity", velocity)?;
                state.end()
            }
            RecordingEventType::NoteOff { midi_note } => {
                let mut state = serializer.serialize_struct("NoteOff", 2)?;
                state.serialize_field("type", "NoteOff")?;
                state.serialize_field("midi_note", midi_note)?;
                state.end()
            }
            RecordingEventType::SustainPedal { pressed } => {
                let mut state = serializer.serialize_struct("SustainPedal", 2)?;
                state.serialize_field("type", "SustainPedal")?;
                state.serialize_field("pressed", pressed)?;
                state.end()
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for RecordingEventType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct RecordingEventTypeVisitor;

        impl<'de> Visitor<'de> for RecordingEventTypeVisitor {
            type Value = RecordingEventType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct RecordingEventType")
            }

            fn visit_map<V>(self, mut map: V) -> Result<RecordingEventType, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut event_type = None;
                let mut midi_note = None;
                let mut velocity = None;
                let mut pressed = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "type" => {
                            if event_type.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            event_type = Some(map.next_value()?);
                        }
                        "midi_note" => {
                            if midi_note.is_some() {
                                return Err(de::Error::duplicate_field("midi_note"));
                            }
                            midi_note = Some(map.next_value()?);
                        }
                        "velocity" => {
                            if velocity.is_some() {
                                return Err(de::Error::duplicate_field("velocity"));
                            }
                            velocity = Some(map.next_value()?);
                        }
                        "pressed" => {
                            if pressed.is_some() {
                                return Err(de::Error::duplicate_field("pressed"));
                            }
                            pressed = Some(map.next_value()?);
                        }
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }

                let event_type: String = event_type.ok_or_else(|| de::Error::missing_field("type"))?;
                match event_type.as_str() {
                    "NoteOn" => {
                        let midi_note = midi_note.ok_or_else(|| de::Error::missing_field("midi_note"))?;
                        let velocity = velocity.ok_or_else(|| de::Error::missing_field("velocity"))?;
                        Ok(RecordingEventType::NoteOn { midi_note, velocity })
                    }
                    "NoteOff" => {
                        let midi_note = midi_note.ok_or_else(|| de::Error::missing_field("midi_note"))?;
                        Ok(RecordingEventType::NoteOff { midi_note })
                    }
                    "SustainPedal" => {
                        let pressed = pressed.ok_or_else(|| de::Error::missing_field("pressed"))?;
                        Ok(RecordingEventType::SustainPedal { pressed })
                    }
                    _ => Err(de::Error::unknown_variant(&event_type, &["NoteOn", "NoteOff", "SustainPedal"])),
                }
            }
        }

        deserializer.deserialize_struct("RecordingEventType", &["type"], RecordingEventTypeVisitor)
    }
}