use ratatui::style::Color;
use std::time::{Duration, Instant};
use rand;

#[derive(Debug, Clone)]
pub struct KeyPressEffect {
    pub start_time: Instant,
    pub duration: Duration,
    #[allow(dead_code)]
    pub color: Color,
    pub intensity: f32,
}

impl KeyPressEffect {
    pub fn new(color: Color) -> Self {
        Self {
            start_time: Instant::now(),
            duration: Duration::from_millis(300),
            color,
            intensity: 1.0,
        }
    }
    
    pub fn is_active(&self) -> bool {
        self.start_time.elapsed() < self.duration
    }
    
    pub fn get_progress(&self) -> f32 {
        let elapsed = self.start_time.elapsed().as_millis() as f32;
        let total = self.duration.as_millis() as f32;
        (elapsed / total).min(1.0)
    }
    
    pub fn get_current_intensity(&self) -> f32 {
        let progress = self.get_progress();
        self.intensity * (1.0 - progress)
    }
}

#[derive(Debug, Clone)]
pub struct ParticleEffect {
    pub x: f32,
    pub y: f32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub color: Color,
    pub lifetime: Duration,
    pub start_time: Instant,
    pub char: char,
}

impl ParticleEffect {
    pub fn new(x: f32, y: f32, color: Color) -> Self {
        Self {
            x,
            y,
            velocity_x: (rand::random::<f32>() - 0.5) * 10.0,
            velocity_y: -rand::random::<f32>() * 5.0 - 2.0,
            color,
            lifetime: Duration::from_millis(1000),
            start_time: Instant::now(),
            char: match rand::random::<u8>() % 4 {
                0 => '♪',
                1 => '♫',
                2 => '♬',
                _ => '♭',
            },
        }
    }
    
    pub fn update(&mut self, dt: f32) {
        self.x += self.velocity_x * dt;
        self.y += self.velocity_y * dt;
        self.velocity_y += 9.8 * dt;
    }
    
    pub fn is_alive(&self) -> bool {
        self.start_time.elapsed() < self.lifetime && self.y < 50.0
    }
    
    pub fn get_alpha(&self) -> f32 {
        let progress = self.start_time.elapsed().as_millis() as f32 / self.lifetime.as_millis() as f32;
        (1.0 - progress).max(0.0)
    }
}

#[derive(Debug)]
pub struct VisualEffects {
    pub key_effects: Vec<(u8, KeyPressEffect)>,
    pub particles: Vec<ParticleEffect>,
    pub glow_effects: Vec<(u16, u16, KeyPressEffect)>,
    pub last_update: Instant,
}

impl VisualEffects {
    pub fn new() -> Self {
        Self {
            key_effects: Vec::new(),
            particles: Vec::new(),
            glow_effects: Vec::new(),
            last_update: Instant::now(),
        }
    }
    
    pub fn add_key_press(&mut self, midi_note: u8, x: u16, y: u16) {
        let color = Self::note_to_color(midi_note);
        
        self.key_effects.push((midi_note, KeyPressEffect::new(color)));
        
        for _ in 0..5 {
            self.particles.push(ParticleEffect::new(x as f32, y as f32, color));
        }
        
        self.glow_effects.push((x, y, KeyPressEffect::new(color)));
    }
    
    pub fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;
        
        self.key_effects.retain(|(_, effect)| effect.is_active());
        
        for particle in &mut self.particles {
            particle.update(dt);
        }
        self.particles.retain(|p| p.is_alive());
        
        self.glow_effects.retain(|(_, _, effect)| effect.is_active());
    }
    
    pub fn get_key_color(&self, midi_note: u8, base_color: Color) -> Color {
        if let Some((_, effect)) = self.key_effects.iter().find(|(note, _)| *note == midi_note) {
            if effect.is_active() {
                let intensity = effect.get_current_intensity();
                // Use the beautiful note-specific color for the keys too!
                let note_color = Self::note_to_color(midi_note);
                return Self::blend_colors(base_color, note_color, intensity * 0.7);
            }
        }
        base_color
    }
    
    #[allow(dead_code)]
    pub fn get_particles_at(&self, x: u16, y: u16, tolerance: u16) -> Vec<&ParticleEffect> {
        let mut result = Vec::new();
        for particle in &self.particles {
            if (particle.x as i32 - x as i32).abs() <= tolerance as i32 && 
               (particle.y as i32 - y as i32).abs() <= tolerance as i32 {
                result.push(particle);
            }
        }
        result
    }
    
    fn note_to_color(midi_note: u8) -> Color {
        // Map each musical note to a specific color
        // Using beautiful tones of oranges, blues, reds, yellows, and greens
        match midi_note % 12 {
            0  => Color::Rgb(255, 85, 85),   // C - Bright Red
            1  => Color::Rgb(255, 140, 70),  // C# - Warm Orange
            2  => Color::Rgb(255, 215, 0),   // D - Golden Yellow
            3  => Color::Rgb(173, 255, 47),  // D# - Green Yellow
            4  => Color::Rgb(50, 205, 50),   // E - Lime Green
            5  => Color::Rgb(0, 191, 165),   // F - Teal Green
            6  => Color::Rgb(70, 130, 255),  // F# - Sky Blue
            7  => Color::Rgb(30, 144, 255),  // G - Dodger Blue
            8  => Color::Rgb(138, 43, 226),  // G# - Blue Violet
            9  => Color::Rgb(255, 20, 147),  // A - Deep Pink
            10 => Color::Rgb(255, 105, 180), // A# - Hot Pink
            11 => Color::Rgb(255, 165, 0),   // B - Orange
            _  => Color::White,              // Fallback
        }
    }
    
    #[allow(dead_code)]
    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
        let h = h % 360.0;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        
        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };
        
        Color::Rgb(
            ((r + m) * 255.0) as u8,
            ((g + m) * 255.0) as u8,
            ((b + m) * 255.0) as u8,
        )
    }
    
    fn blend_colors(base: Color, overlay: Color, intensity: f32) -> Color {
        match (base, overlay) {
            (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
                let blend = |c1: u8, c2: u8| {
                    let f1 = c1 as f32 / 255.0;
                    let f2 = c2 as f32 / 255.0;
                    let blended = f1 * (1.0 - intensity) + f2 * intensity;
                    (blended * 255.0) as u8
                };
                
                Color::Rgb(
                    blend(r1, r2),
                    blend(g1, g2),
                    blend(b1, b2),
                )
            }
            _ => overlay,
        }
    }
}

#[allow(dead_code)]
pub struct SimpleEffects {
    pub enabled: bool,
}

#[allow(dead_code)]
impl SimpleEffects {
    pub fn new() -> Self {
        Self {
            enabled: true,
        }
    }
    
    pub fn apply_glow_effect(&self, base_color: Color, intensity: f32) -> Color {
        if !self.enabled || intensity <= 0.0 {
            return base_color;
        }
        
        match base_color {
            Color::Rgb(r, g, b) => {
                let factor = 1.0 + (intensity * 0.5);
                Color::Rgb(
                    ((r as f32 * factor).min(255.0)) as u8,
                    ((g as f32 * factor).min(255.0)) as u8,
                    ((b as f32 * factor).min(255.0)) as u8,
                )
            }
            _ => base_color,
        }
    }
}