use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteType {
    White,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Note {
    pub midi_note: u8,
    pub octave: u8,
    pub note_name: NoteName,
    pub note_type: NoteType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteName {
    C, CSharp, D, DSharp, E, F, FSharp, G, GSharp, A, ASharp, B,
}

impl NoteName {
    pub fn from_midi(midi_note: u8) -> Self {
        match midi_note % 12 {
            0 => NoteName::C,
            1 => NoteName::CSharp,
            2 => NoteName::D,
            3 => NoteName::DSharp,
            4 => NoteName::E,
            5 => NoteName::F,
            6 => NoteName::FSharp,
            7 => NoteName::G,
            8 => NoteName::GSharp,
            9 => NoteName::A,
            10 => NoteName::ASharp,
            11 => NoteName::B,
            _ => unreachable!(),
        }
    }
    
    pub fn is_black_key(&self) -> bool {
        matches!(self, NoteName::CSharp | NoteName::DSharp | NoteName::FSharp | NoteName::GSharp | NoteName::ASharp)
    }
    
    pub fn to_string(&self) -> &'static str {
        match self {
            NoteName::C => "C",
            NoteName::CSharp => "C#",
            NoteName::D => "D",
            NoteName::DSharp => "D#",
            NoteName::E => "E",
            NoteName::F => "F",
            NoteName::FSharp => "F#",
            NoteName::G => "G",
            NoteName::GSharp => "G#",
            NoteName::A => "A",
            NoteName::ASharp => "A#",
            NoteName::B => "B",
        }
    }
}

impl Note {
    pub fn new(midi_note: u8) -> Self {
        let note_name = NoteName::from_midi(midi_note);
        let octave = midi_note / 12;
        let note_type = if note_name.is_black_key() {
            NoteType::Black
        } else {
            NoteType::White
        };
        
        Self {
            midi_note,
            octave,
            note_name,
            note_type,
        }
    }
    
    pub fn frequency(&self) -> f32 {
        440.0 * 2.0_f32.powf((self.midi_note as f32 - 69.0) / 12.0)
    }
}

#[derive(Debug)]
pub struct Piano {
    pub current_octave: u8,
    pub pressed_keys: HashMap<u8, Instant>, // Track when keys were pressed
    pub sustain_pedal: bool,
    pub volume: f32,
    pub key_mappings: HashMap<char, u8>,
}

impl Piano {
    pub fn new() -> Self {
        let mut piano = Self {
            current_octave: 4,
            pressed_keys: HashMap::new(),
            sustain_pedal: false,
            volume: 0.7,
            key_mappings: HashMap::new(),
        };
        
        piano.setup_key_mappings();
        piano
    }
    
    fn setup_key_mappings(&mut self) {
        self.key_mappings.clear();
        
        // Map keyboard keys to piano notes in a logical way
        // White keys: q w e r t y u i o p for one octave
        // Black keys: 2 3 5 6 7 9 0 for sharps/flats
        let white_keys = ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'];
        let black_keys = ['2', '3', '5', '6', '7', '9', '0'];
        
        // Second octave row
        let white_keys_2 = ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';'];
        let black_keys_2 = ['1', '4', '8', '-', '='];
        
        let base_octave = self.current_octave * 12;
        
        // Map first octave
        let mut white_index = 0;
        let mut black_index = 0;
        
        for note_offset in 0..12 {
            let midi_note = base_octave + note_offset;
            if midi_note > 127 { break; }
            
            let note = Note::new(midi_note);
            
            match note.note_type {
                NoteType::White => {
                    if white_index < white_keys.len() {
                        self.key_mappings.insert(white_keys[white_index], midi_note);
                        white_index += 1;
                    }
                }
                NoteType::Black => {
                    if black_index < black_keys.len() {
                        self.key_mappings.insert(black_keys[black_index], midi_note);
                        black_index += 1;
                    }
                }
            }
        }
        
        // Map second octave if space allows
        white_index = 0;
        black_index = 0;
        
        for note_offset in 12..24 {
            let midi_note = base_octave + note_offset;
            if midi_note > 127 { break; }
            
            let note = Note::new(midi_note);
            
            match note.note_type {
                NoteType::White => {
                    if white_index < white_keys_2.len() {
                        self.key_mappings.insert(white_keys_2[white_index], midi_note);
                        white_index += 1;
                    }
                }
                NoteType::Black => {
                    if black_index < black_keys_2.len() {
                        self.key_mappings.insert(black_keys_2[black_index], midi_note);
                        black_index += 1;
                    }
                }
            }
        }
        
        // Add some special mappings for common keys
        self.key_mappings.insert('z', base_octave.saturating_sub(12)); // Lower C
        self.key_mappings.insert('x', base_octave.saturating_sub(10)); // Lower D
        self.key_mappings.insert('c', base_octave.saturating_sub(8));  // Lower E
        self.key_mappings.insert('v', base_octave.saturating_sub(7));  // Lower F
        self.key_mappings.insert('b', base_octave.saturating_sub(5));  // Lower G
        self.key_mappings.insert('n', base_octave.saturating_sub(3));  // Lower A
        self.key_mappings.insert('m', base_octave.saturating_sub(1));  // Lower B
    }
    
    pub fn press_key(&mut self, midi_note: u8) {
        self.pressed_keys.insert(midi_note, Instant::now());
    }
    
    pub fn release_key(&mut self, midi_note: u8) {
        if !self.sustain_pedal {
            self.pressed_keys.remove(&midi_note);
        }
    }
    
    pub fn toggle_sustain(&mut self) {
        self.sustain_pedal = !self.sustain_pedal;
        if !self.sustain_pedal {
            self.pressed_keys.clear();
        }
    }
    
    pub fn change_octave(&mut self, delta: i8) {
        let new_octave = (self.current_octave as i8 + delta).clamp(0, 8) as u8;
        if new_octave != self.current_octave {
            self.current_octave = new_octave;
            self.setup_key_mappings();
        }
    }
    
    pub fn adjust_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
    }
    
    pub fn get_midi_note_from_key(&self, key: char) -> Option<u8> {
        self.key_mappings.get(&key).copied()
    }
    
    #[allow(dead_code)]
    pub fn get_octave_range(&self) -> (u8, u8) {
        let start = self.current_octave * 12;
        (start, start + 12)
    }
    
    pub fn update(&mut self) {
        // Auto-release keys after 300ms if not using sustain pedal
        if !self.sustain_pedal {
            let now = Instant::now();
            self.pressed_keys.retain(|_, &mut press_time| {
                now.duration_since(press_time).as_millis() < 300
            });
        }
    }
    
    #[allow(dead_code)]
    pub fn get_key_layout(&self) -> Vec<(char, Note, bool)> {
        let mut layout = Vec::new();
        
        for (&key, &midi_note) in &self.key_mappings {
            let note = Note::new(midi_note);
            let is_pressed = self.pressed_keys.contains_key(&midi_note);
            layout.push((key, note, is_pressed));
        }
        
        layout.sort_by_key(|(_, note, _)| note.midi_note);
        layout
    }
}

#[derive(Debug, Clone)]
pub struct PianoLayout {
    pub white_keys: Vec<WhiteKey>,
    pub black_keys: Vec<BlackKey>,
    #[allow(dead_code)]
    pub width: u16,
    #[allow(dead_code)]
    pub height: u16,
}

#[derive(Debug, Clone)]
pub struct WhiteKey {
    pub note: Note,
    pub x: u16,
    pub width: u16,
    pub is_pressed: bool,
    pub key_char: Option<char>,
}

#[derive(Debug, Clone)]
pub struct BlackKey {
    pub note: Note,
    pub x: u16,
    pub width: u16,
    pub is_pressed: bool,
    pub key_char: Option<char>,
}

impl PianoLayout {
    pub fn new(piano: &Piano, terminal_width: u16) -> Self {
        // Calculate how many octaves we can fit in the full terminal width
        let white_keys_per_octave = 7;
        let usable_width = terminal_width.saturating_sub(4); // Leave small margin for borders
        let min_key_width = 6; // Minimum width for readability
        
        // Calculate maximum octaves that fit with minimum key width
        let max_white_keys = usable_width / min_key_width;
        let max_octaves = std::cmp::max(2, max_white_keys / white_keys_per_octave); // At least 2 octaves
        let octave_count = std::cmp::min(7, max_octaves); // Max 7 octaves (full piano range)
        
        let total_white_keys = octave_count * white_keys_per_octave;
        let key_width = usable_width / total_white_keys; // Distribute evenly across full width
        let black_key_width = std::cmp::max(3, (key_width as f32 * 0.6) as u16);
        
        let mut white_keys = Vec::new();
        let mut black_keys = Vec::new();
        
        let key_mappings = &piano.key_mappings;
        let char_to_key: HashMap<u8, char> = key_mappings.iter().map(|(&k, &v)| (v, k)).collect();
        
        // Start from the left edge with margin for borders
        let offset_x = 2;
        
        let mut white_key_index = 0;
        
        // Start from C3 and show the calculated number of octaves
        let start_midi_note = 48; // C3
        let total_notes = octave_count * 12;
        
        for midi_note in start_midi_note..(start_midi_note + total_notes).min(127) {
            let midi_note = midi_note as u8;
            let note = Note::new(midi_note);
            let is_pressed = piano.pressed_keys.contains_key(&midi_note);
            let key_char = char_to_key.get(&midi_note).copied();
            
            match note.note_type {
                NoteType::White => {
                    white_keys.push(WhiteKey {
                        note,
                        x: offset_x + (white_key_index * key_width),
                        width: key_width,
                        is_pressed,
                        key_char,
                    });
                    white_key_index += 1;
                }
                NoteType::Black => {
                    // Position black keys between white keys
                    let white_key_x = if white_key_index > 0 {
                        offset_x + ((white_key_index - 1) * key_width)
                    } else {
                        offset_x
                    };
                    
                    // Center the black key between current and next white key
                    let black_x = white_key_x + key_width - (black_key_width / 2);
                    
                    black_keys.push(BlackKey {
                        note,
                        x: black_x,
                        width: black_key_width,
                        is_pressed,
                        key_char,
                    });
                }
            }
        }
        
        Self {
            white_keys,
            black_keys,
            width: terminal_width, // Use full terminal width
            height: 12, // Taller piano for better presence
        }
    }
}