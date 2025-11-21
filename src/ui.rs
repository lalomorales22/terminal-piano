use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap},
};

use crate::{
    piano::{Piano, PianoLayout},
    effects::VisualEffects,
    midi::MidiPlayer,
    audio::AudioEngine,
};

pub struct UI {
    pub show_help: bool,
    #[allow(dead_code)]
    pub show_info: bool,
    #[allow(dead_code)]
    pub current_octave_display: u8,
    #[allow(dead_code)]
    pub volume_display: f32,
    pub recording: bool,
    pub metronome: bool,
    pub status_message: Option<String>,
}

impl UI {
    pub fn new() -> Self {
        Self {
            show_help: false,
            show_info: true,
            current_octave_display: 4,
            volume_display: 0.7,
            recording: false,
            metronome: false,
            status_message: None,
        }
    }
    
    pub fn render(
        &mut self,
        f: &mut ratatui::Frame,
        piano: &Piano,
        effects: &VisualEffects,
        midi_player: &MidiPlayer,
        audio_engine: &AudioEngine,
    ) {
        let size = f.area();
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Length(2),  // MIDI Progress (when playing)
                Constraint::Min(13),    // Piano - still plenty of space
                Constraint::Length(3),  // Controls
                Constraint::Length(1),  // Status
            ])
            .split(size);
        
        self.render_header(f, chunks[0], piano, midi_player, audio_engine);
        self.render_midi_progress(f, chunks[1], midi_player);
        self.render_piano(f, chunks[2], piano, effects);
        self.render_controls(f, chunks[3], piano);
        self.render_status(f, chunks[4]);
        
        if self.show_help {
            self.render_help_popup(f, size);
        }
    }
    
    fn render_header(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        piano: &Piano,
        midi_player: &MidiPlayer,
        _audio_engine: &AudioEngine,
    ) {
        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25), // Title
                Constraint::Percentage(25), // Octave
                Constraint::Percentage(25), // Volume
                Constraint::Percentage(25), // Status
            ])
            .split(area);
        
        let title = Paragraph::new("🎹 Terminal Piano")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, header_chunks[0]);
        
        let octave_text = format!("Octave: {}", piano.current_octave);
        let octave = Paragraph::new(octave_text)
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(octave, header_chunks[1]);
        
        let volume_gauge = Gauge::default()
            .block(Block::default().title("Volume").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(piano.volume as f64);
        f.render_widget(volume_gauge, header_chunks[2]);
        
        let mut status_spans = vec![
            Span::styled(
                if self.recording { "REC " } else { "" },
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if self.metronome { "♩ " } else { "" },
                Style::default().fg(Color::Blue),
            ),
            Span::styled(
                if piano.sustain_pedal { "SUS " } else { "" },
                Style::default().fg(Color::Magenta),
            ),
        ];
        
        if let Some(current_file) = &midi_player.current_file {
            status_spans.push(Span::styled(
                format!("♪ {}", current_file.file_name().unwrap_or_default().to_string_lossy()),
                Style::default().fg(Color::Cyan),
            ));
        }
        
        let status = Paragraph::new(Line::from(status_spans))
            .alignment(Alignment::Center)
            .block(Block::default().title("Status").borders(Borders::ALL));
        f.render_widget(status, header_chunks[3]);
    }
    
    fn render_midi_progress(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        midi_player: &MidiPlayer,
    ) {
        if midi_player.current_file.is_some() {
            let progress = midi_player.get_progress();
            let (current_time, total_time) = midi_player.get_time_info();
            
            let title = if midi_player.is_playing {
                format!("♪ Playing - {:02}:{:02} / {:02}:{:02}", 
                    current_time.as_secs() / 60, current_time.as_secs() % 60,
                    total_time.as_secs() / 60, total_time.as_secs() % 60)
            } else {
                format!("♪ Paused - {:02}:{:02} / {:02}:{:02}", 
                    current_time.as_secs() / 60, current_time.as_secs() % 60,
                    total_time.as_secs() / 60, total_time.as_secs() % 60)
            };
            
            let gauge_color = if midi_player.is_playing { Color::Green } else { Color::Yellow };
            
            let progress_gauge = Gauge::default()
                .block(Block::default().title(title).borders(Borders::ALL))
                .gauge_style(Style::default().fg(gauge_color))
                .ratio(progress as f64);
            f.render_widget(progress_gauge, area);
        } else {
            // Show empty space when no MIDI file is loaded
            let empty_block = Block::default()
                .title("No MIDI file loaded - Press L to load")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray));
            f.render_widget(empty_block, area);
        }
    }
    
    fn render_piano(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        piano: &Piano,
        effects: &VisualEffects,
    ) {
        let piano_layout = PianoLayout::new(piano, area.width);
        
        let piano_block = Block::default()
            .title("Piano")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White));
        let inner_area = piano_block.inner(area);
        f.render_widget(piano_block, area);
        
        self.render_white_keys(f, inner_area, &piano_layout, effects);
        self.render_black_keys(f, inner_area, &piano_layout, effects);
        self.render_particles(f, inner_area, effects);
    }
    
    fn render_white_keys(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        layout: &PianoLayout,
        effects: &VisualEffects,
    ) {
        for white_key in &layout.white_keys {
            if white_key.x + white_key.width > area.width {
                break;
            }
            
            let key_area = Rect {
                x: area.x + white_key.x,
                y: area.y,
                width: white_key.width,
                height: area.height,
            };
            
            let base_color = if white_key.is_pressed {
                Color::Gray
            } else {
                Color::White
            };
            
            let color = effects.get_key_color(white_key.note.midi_note, base_color);
            
            let key_content = if let Some(key_char) = white_key.key_char {
                format!("│{:^width$}│", key_char, width = (white_key.width.saturating_sub(2)) as usize)
            } else {
                format!("│{:^width$}│", "", width = (white_key.width.saturating_sub(2)) as usize)
            };
            
            // Create a taller key with more lines to fill the piano area better
            let mut lines = vec![
                "┌".to_string() + &"─".repeat((white_key.width.saturating_sub(2)) as usize) + "┐",
                key_content,
                format!("│{:^width$}│", white_key.note.note_name.to_string(), width = (white_key.width.saturating_sub(2)) as usize),
            ];
            
            // Add more lines to make keys taller and fill the space
            for _ in 0..(area.height.saturating_sub(4)) {
                lines.push("│".to_string() + &" ".repeat((white_key.width.saturating_sub(2)) as usize) + "│");
            }
            
            lines.push("└".to_string() + &"─".repeat((white_key.width.saturating_sub(2)) as usize) + "┘");
            
            let key_widget = Paragraph::new(Text::from(
                lines.into_iter().map(Line::from).collect::<Vec<_>>()
            ))
            .style(Style::default().fg(Color::Black).bg(color));
            
            f.render_widget(key_widget, key_area);
        }
    }
    
    fn render_black_keys(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        layout: &PianoLayout,
        effects: &VisualEffects,
    ) {
        for black_key in &layout.black_keys {
            if black_key.x + black_key.width > area.width {
                break;
            }
            
            let key_height = area.height * 2 / 3;
            let key_area = Rect {
                x: area.x + black_key.x,
                y: area.y,
                width: black_key.width,
                height: key_height,
            };
            
            let base_color = if black_key.is_pressed {
                Color::DarkGray
            } else {
                Color::Black
            };
            
            let color = effects.get_key_color(black_key.note.midi_note, base_color);
            
            let key_content = if let Some(key_char) = black_key.key_char {
                format!("│{:^width$}│", key_char, width = (black_key.width.saturating_sub(2)) as usize)
            } else {
                format!("│{:^width$}│", "", width = (black_key.width.saturating_sub(2)) as usize)
            };
            
            // Create black keys that are proportionally tall
            let mut lines = vec![
                "┌".to_string() + &"─".repeat((black_key.width.saturating_sub(2)) as usize) + "┐",
                key_content,
                format!("│{:^width$}│", black_key.note.note_name.to_string(), width = (black_key.width.saturating_sub(2)) as usize),
            ];
            
            // Add lines to make black keys taller but still shorter than white keys
            for _ in 0..(key_height.saturating_sub(4)) {
                lines.push("│".to_string() + &" ".repeat((black_key.width.saturating_sub(2)) as usize) + "│");
            }
            
            lines.push("└".to_string() + &"─".repeat((black_key.width.saturating_sub(2)) as usize) + "┘");
            
            let key_widget = Paragraph::new(Text::from(
                lines.into_iter().map(Line::from).collect::<Vec<_>>()
            ))
            .style(Style::default().fg(Color::White).bg(color));
            
            f.render_widget(key_widget, key_area);
        }
    }
    
    fn render_particles(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        effects: &VisualEffects,
    ) {
        for particle in &effects.particles {
            let x = particle.x as u16;
            let y = particle.y as u16;
            
            if x < area.width && y < area.height {
                let particle_area = Rect {
                    x: area.x + x,
                    y: area.y + y,
                    width: 1,
                    height: 1,
                };
                
                let _alpha = particle.get_alpha();
                let color = particle.color;
                
                let particle_widget = Paragraph::new(particle.char.to_string())
                    .style(Style::default().fg(color));
                
                f.render_widget(particle_widget, particle_area);
            }
        }
    }
    
    fn render_controls(&self, f: &mut ratatui::Frame, area: Rect, _piano: &Piano) {
        let controls_text = vec![
            Line::from(vec![
                Span::styled("Controls: ", Style::default().fg(Color::Yellow)),
                Span::raw("[ / ] Volume "),
                Span::raw("+ Octave Up "),
                Span::raw("_ Octave Down "),
                Span::raw("Space Sustain "),
                Span::raw("R Record "),
                Span::raw("P Play "),
                Span::raw("M Metronome "),
                Span::raw("L Load "),
                Span::raw("F1 Help "),
                Span::raw("Q Quit"),
            ]),
            Line::from(vec![
                Span::styled("Piano Keys: ", Style::default().fg(Color::Green)),
                Span::raw("ASDFGHJKL;... for white keys, 1234567890-= for black keys"),
            ]),
        ];
        
        let controls = Paragraph::new(controls_text)
            .block(Block::default().title("Controls").borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        f.render_widget(controls, area);
    }
    
    fn render_status(&self, f: &mut ratatui::Frame, area: Rect) {
        let status_text = if let Some(ref msg) = self.status_message {
            msg.clone()
        } else {
            "Ready - Press F1 for help".to_string()
        };
        
        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(status, area);
    }
    
    fn render_help_popup(&self, f: &mut ratatui::Frame, area: Rect) {
        let popup_area = centered_rect(60, 70, area);
        
        f.render_widget(Clear, popup_area);
        
        let help_text = vec![
            Line::from("🎹 Terminal Piano Help"),
            Line::from(""),
            Line::from("Piano Keys:"),
            Line::from("  White keys: A S D F G H J K L ; Z X C V B N M , . /"),
            Line::from("  Black keys: 1 2 3 4 5 6 7 8 9 0 - ="),
            Line::from(""),
            Line::from("Controls:"),
            Line::from("  [ / ]     - Volume down/up"),
            Line::from("  + / _     - Octave up/down"),
            Line::from("  Space     - Sustain pedal"),
            Line::from("  R         - Start/stop recording"),
            Line::from("  P (upper) - Toggle MIDI playback (with key lighting)"),
            Line::from("  p (lower) - Playback last recording"),
            Line::from("  M         - Toggle metronome"),
            Line::from("  L         - Load MIDI file"),
            Line::from("  F1        - Toggle this help"),
            Line::from("  Q         - Quit"),
            Line::from(""),
            Line::from("Press any key to close this help..."),
        ];
        
        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });
        
        f.render_widget(help, popup_area);
    }
    
    pub fn set_status_message(&mut self, message: String) {
        self.status_message = Some(message);
    }
    
    #[allow(dead_code)]
    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }
    
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}