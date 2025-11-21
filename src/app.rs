use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use rand;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::{
    audio::{AudioEngine, Recording, RecordingEventType},
    config::Config,
    effects::VisualEffects,
    file_dialog::FileDialog,
    midi::{MidiPlayer, MidiRecorder},
    piano::Piano,
    ui::UI,
};

pub struct App {
    pub piano: Piano,
    pub audio_engine: AudioEngine,
    pub midi_player: MidiPlayer,
    pub midi_recorder: MidiRecorder,
    pub visual_effects: VisualEffects,
    pub ui: UI,
    pub config: Config,
    pub debug_mode: bool,
    pub should_quit: bool,
    pub last_update: Instant,
}

impl App {
    pub async fn new(debug_mode: bool) -> Result<Self> {
        let config = Config::load()?;
        let audio_engine = AudioEngine::new()?;
        let piano = Piano::new();
        let midi_player = MidiPlayer::new();
        let midi_recorder = MidiRecorder::new();
        let visual_effects = VisualEffects::new();
        let ui = UI::new();

        Ok(Self {
            piano,
            audio_engine,
            midi_player,
            midi_recorder,
            visual_effects,
            ui,
            config,
            debug_mode,
            should_quit: false,
            last_update: Instant::now(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal).await;

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        if let Err(err) = result {
            println!("Error: {:?}", err);
        }

        Ok(())
    }

    async fn run_app<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let mut last_tick = Instant::now();
        let tick_rate = Duration::from_millis(16); // ~60 FPS

        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match key.kind {
                        crossterm::event::KeyEventKind::Press => {
                            self.handle_key_event(key).await?;
                        }
                        crossterm::event::KeyEventKind::Release => {
                            self.handle_key_release(key).await?;
                        }
                        _ => {}
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.update().await?;
                terminal.draw(|f| self.render(f))?;
                last_tick = Instant::now();
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if self.ui.show_help {
            self.ui.toggle_help();
            return Ok(());
        }

        // Debug ALL key presses
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/piano_debug.log") {
            writeln!(file, "KEY PRESSED: {:?} with modifiers: {:?}", key.code, key.modifiers).ok();
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => {
                self.should_quit = true;
            }
            (KeyCode::Char('Q'), KeyModifiers::NONE) => {
                self.should_quit = true;
            }
            (KeyCode::F(1), KeyModifiers::NONE) => {
                self.ui.toggle_help();
            }
            (KeyCode::Char('['), KeyModifiers::NONE) => {
                self.piano.adjust_volume(-0.1);
                self.audio_engine.set_volume(self.piano.volume);
                self.ui.set_status_message(format!("Volume: {:.0}%", self.piano.volume * 100.0));
            }
            (KeyCode::Char(']'), KeyModifiers::NONE) => {
                self.piano.adjust_volume(0.1);
                self.audio_engine.set_volume(self.piano.volume);
                self.ui.set_status_message(format!("Volume: {:.0}%", self.piano.volume * 100.0));
            }
            (KeyCode::Char('-'), KeyModifiers::NONE) => {
                if let Some(midi_note) = self.piano.get_midi_note_from_key('-') {
                    self.play_note(midi_note).await?;
                }
            }
            (KeyCode::Char('+'), KeyModifiers::NONE) => {
                self.piano.change_octave(1);
                self.ui.set_status_message(format!("Octave: {}", self.piano.current_octave));
            }
            (KeyCode::Char('='), KeyModifiers::NONE) => {
                if let Some(midi_note) = self.piano.get_midi_note_from_key('=') {
                    self.play_note(midi_note).await?;
                }
            }
            (KeyCode::Char('_'), KeyModifiers::NONE) => {
                self.piano.change_octave(-1);
                self.ui.set_status_message(format!("Octave: {}", self.piano.current_octave));
            }
            (KeyCode::Char(' '), KeyModifiers::NONE) => {
                self.piano.toggle_sustain();
                self.midi_recorder.record_sustain_pedal(self.piano.sustain_pedal);
                self.ui.set_status_message(format!("Sustain: {}", if self.piano.sustain_pedal { "ON" } else { "OFF" }));
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                if let Some(recording) = self.midi_recorder.toggle_recording() {
                    self.save_recording(recording).await?;
                    self.ui.set_status_message("Recording saved".to_string());
                } else {
                    self.ui.set_status_message("Recording started".to_string());
                }
                self.ui.recording = self.midi_recorder.is_recording;
            }
            (KeyCode::Char('R'), KeyModifiers::NONE) => {
                if let Some(recording) = self.midi_recorder.toggle_recording() {
                    self.save_recording(recording).await?;
                    self.ui.set_status_message("Recording saved".to_string());
                } else {
                    self.ui.set_status_message("Recording started".to_string());
                }
                self.ui.recording = self.midi_recorder.is_recording;
            }
            (KeyCode::Char('p'), KeyModifiers::NONE) => {
                self.load_last_recording().await?;
            }
            (KeyCode::Char('P'), KeyModifiers::SHIFT) => {
                // Debug key press
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/piano_debug.log") {
                    writeln!(file, "CAPITAL P PRESSED - current_file: {:?}, events: {}", 
                            self.midi_player.current_file.as_ref().map(|p| p.file_name().unwrap_or_default()),
                            self.midi_player.events.len()).ok();
                }
                
                if self.midi_player.current_file.is_some() {
                    let events_count = self.midi_player.events.len();
                    let total_ticks = self.midi_player.total_ticks;
                    let was_playing = self.midi_player.is_playing;
                    
                    self.midi_player.toggle_playback();
                    
                    let status = if self.midi_player.is_playing { "Playing" } else { "Paused" };
                    
                    // More debug
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("/tmp/piano_debug.log") {
                        writeln!(file, "TOGGLE RESULT - was_playing: {}, now_playing: {}, events: {}", 
                                was_playing, self.midi_player.is_playing, events_count).ok();
                    }
                    
                    self.ui.set_status_message(format!("MIDI {}: {} events, {} ticks", status, events_count, total_ticks));
                } else {
                    self.ui.set_status_message("No MIDI file loaded. Press 'L' to load a file.".to_string());
                }
            }
            (KeyCode::Char('m'), KeyModifiers::NONE) => {
                self.ui.metronome = !self.ui.metronome;
                self.ui.set_status_message(format!("Metronome: {}", if self.ui.metronome { "ON" } else { "OFF" }));
            }
            (KeyCode::Char('M'), KeyModifiers::NONE) => {
                self.ui.metronome = !self.ui.metronome;
                self.ui.set_status_message(format!("Metronome: {}", if self.ui.metronome { "ON" } else { "OFF" }));
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) => {
                self.load_midi_file_dialog().await?;
            }
            (KeyCode::Char('L'), KeyModifiers::NONE) => {
                self.load_midi_file_dialog().await?;
            }
            (KeyCode::Char(c), KeyModifiers::NONE) => {
                if let Some(midi_note) = self.piano.get_midi_note_from_key(c) {
                    self.play_note(midi_note).await?;
                }
            }
            (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                if let Some(midi_note) = self.piano.get_midi_note_from_key(c) {
                    self.play_note(midi_note).await?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_key_release(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        // Only handle piano key releases, not control keys
        match key.code {
            KeyCode::Char(c) => {
                if let Some(midi_note) = self.piano.get_midi_note_from_key(c) {
                    self.release_note(midi_note).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn play_note(&mut self, midi_note: u8) -> Result<()> {
        self.piano.press_key(midi_note);
        self.audio_engine.play_note(midi_note)?;
        self.midi_recorder.record_note_on(midi_note, 127);
        
        let (x, y) = self.get_key_position(midi_note);
        self.visual_effects.add_key_press(midi_note, x, y);
        
        Ok(())
    }

    async fn play_midi_note(&mut self, midi_note: u8, velocity: u8) -> Result<()> {
        self.piano.press_key(midi_note);
        self.audio_engine.play_note(midi_note)?;
        
        let (x, y) = self.get_key_position(midi_note);
        
        // Add prominent visual effects for MIDI playback with intensity based on velocity
        self.visual_effects.add_key_press(midi_note, x, y);
        
        // Create spectacular particle effects for MIDI notes
        // More particles for louder notes (higher velocity)
        let particle_count = 3 + (velocity / 32) as usize; // 3-6 particles based on velocity
        
        for _ in 0..particle_count {
            let offset_x = x + (rand::random::<u16>() % 8).saturating_sub(4); // Spread around key
            let offset_y = y.saturating_sub(rand::random::<u16>() % 3); // Slightly above key
            self.visual_effects.add_key_press(midi_note, offset_x, offset_y);
        }
        
        // Add extra burst for loud notes
        if velocity > 100 {
            for _ in 0..3 {
                let burst_x = x + (rand::random::<u16>() % 12).saturating_sub(6);
                let burst_y = y.saturating_sub(rand::random::<u16>() % 5);
                self.visual_effects.add_key_press(midi_note, burst_x, burst_y);
            }
        }
        
        Ok(())
    }

    async fn release_note(&mut self, midi_note: u8) -> Result<()> {
        self.piano.release_key(midi_note);
        self.audio_engine.stop_note(midi_note);
        self.midi_recorder.record_note_off(midi_note);
        Ok(())
    }

    fn get_key_position(&self, midi_note: u8) -> (u16, u16) {
        // Calculate key position based on MIDI note for better visual effects
        let note_in_octave = midi_note % 12;
        let _octave = midi_note / 12;
        
        // Base position calculation - approximate position in the current layout
        let white_key_positions = [0, 2, 4, 5, 7, 9, 11]; // C, D, E, F, G, A, B positions
        let black_key_positions = [1, 3, 6, 8, 10]; // C#, D#, F#, G#, A# positions
        
        let is_black_key = matches!(note_in_octave, 1 | 3 | 6 | 8 | 10);
        
        if is_black_key {
            // For black keys, position them between white keys
            let black_index = black_key_positions.iter().position(|&x| x == note_in_octave).unwrap_or(0);
            let x = 15 + (black_index as u16 * 12); // Approximate spacing
            let y = 3; // Black keys are higher
            (x, y)
        } else {
            // For white keys, spread them evenly
            let white_index = white_key_positions.iter().position(|&x| x == note_in_octave).unwrap_or(0);
            let x = 10 + (white_index as u16 * 10); // Approximate spacing for white keys
            let y = 8; // White keys are lower
            (x, y)
        }
    }

    async fn update(&mut self) -> Result<()> {
        let now = Instant::now();
        let _dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        self.visual_effects.update();
        self.audio_engine.cleanup_finished_notes();
        self.piano.update(); // Auto-release keys after timeout

        let pending_midi_events = self.midi_player.get_pending_events();
        if !pending_midi_events.is_empty() && self.debug_mode {
            self.ui.set_status_message(format!("Processing {} MIDI events", pending_midi_events.len()));
        }
        for event in pending_midi_events {
            match event {
                midly::MidiMessage::NoteOn { key, vel } => {
                    if vel.as_int() > 0 {
                        self.play_midi_note(key.as_int(), vel.as_int()).await?;
                    } else {
                        self.release_note(key.as_int()).await?;
                    }
                }
                midly::MidiMessage::NoteOff { key, vel: _ } => {
                    self.release_note(key.as_int()).await?;
                }
                _ => {}
            }
        }

        if self.ui.status_message.is_some() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    fn render(&mut self, f: &mut ratatui::Frame) {
        self.ui.render(f, &self.piano, &self.visual_effects, &self.midi_player, &self.audio_engine);
    }

    pub async fn load_midi_file(&mut self, path: PathBuf) -> Result<()> {
        self.midi_player.load_file(&path)?;
        self.ui.set_status_message(format!("Loaded: {}", path.file_name().unwrap_or_default().to_string_lossy()));
        Ok(())
    }
    
    async fn load_midi_file_dialog(&mut self) -> Result<()> {
        if let Ok(Some(path)) = FileDialog::open_file() {
            self.load_midi_file(path).await?;
        } else {
            self.ui.set_status_message("No file selected or file dialog unavailable".to_string());
        }
        Ok(())
    }

    async fn save_recording(&self, recording: Recording) -> Result<()> {
        let recordings_dir = crate::config::Config::recordings_dir()?;
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("recording_{}.json", timestamp);
        let path = recordings_dir.join(filename);
        recording.save_to_file(&path)?;
        Ok(())
    }

    async fn load_last_recording(&mut self) -> Result<()> {
        let recordings_dir = crate::config::Config::recordings_dir()?;
        
        if let Ok(entries) = std::fs::read_dir(&recordings_dir) {
            let mut recordings: Vec<_> = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.path().extension().map_or(false, |ext| ext == "json")
                })
                .collect();
            
            recordings.sort_by_key(|entry| {
                entry.metadata().unwrap().modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });
            
            if let Some(last_recording) = recordings.last() {
                let recording = Recording::load_from_file(&last_recording.path())?;
                self.playback_recording(recording).await?;
                self.ui.set_status_message("Playing back last recording".to_string());
            } else {
                self.ui.set_status_message("No recordings found".to_string());
            }
        } else {
            self.ui.set_status_message("No recordings directory found".to_string());
        }
        
        Ok(())
    }

    async fn playback_recording(&mut self, recording: Recording) -> Result<()> {
        let start_time = Instant::now();
        
        for event in recording.events {
            let target_time = start_time + event.timestamp;
            let now = Instant::now();
            
            if target_time > now {
                tokio::time::sleep(target_time - now).await;
            }
            
            match event.event_type {
                RecordingEventType::NoteOn { midi_note, velocity: _ } => {
                    self.play_note(midi_note).await?;
                }
                RecordingEventType::NoteOff { midi_note } => {
                    self.release_note(midi_note).await?;
                }
                RecordingEventType::SustainPedal { pressed } => {
                    if pressed != self.piano.sustain_pedal {
                        self.piano.toggle_sustain();
                    }
                }
            }
        }
        
        Ok(())
    }

    pub fn show_config(&self) -> Result<()> {
        println!("Terminal Piano Configuration:");
        println!("Audio:");
        println!("  Sample Rate: {} Hz", self.config.audio.sample_rate);
        println!("  Buffer Size: {}", self.config.audio.buffer_size);
        println!("  Volume: {:.0}%", self.config.audio.volume * 100.0);
        println!("  Sound Font: {:?}", self.config.audio.sound_font);
        
        println!("UI:");
        println!("  Color Scheme: {}", self.config.ui.color_scheme);
        println!("  Show Notes: {}", self.config.ui.show_notes);
        println!("  Show Keyboard Hints: {}", self.config.ui.show_keyboard_hints);
        println!("  Animation Speed: {}", self.config.ui.animation_speed);
        
        println!("MIDI:");
        println!("  Input Device: {}", self.config.midi.input_device);
        println!("  Output Device: {}", self.config.midi.output_device);
        
        Ok(())
    }
}