use macroquad::prelude::*;

const MAX_GAUSSIANS: usize = 8;

pub struct GaussianDist {
    pub mean: f32,
    pub sdev: f32,
    pub ampl: f32,
}

pub struct Spectrum {
    pub base: f32,
    pub adds: Vec<GaussianDist>,
    pub subs: Vec<GaussianDist>,
}

pub struct SpectrumRenderer {
    material:   Material,
    texture:    Texture2D,
    freq_min:   f32,
    freq_max:   f32,
    ampl_max:   f32,
    base:       f32,
    adds_count: i32,
    subs_count: i32,
}

impl SpectrumRenderer {
    pub async fn new(freq_min: f32, freq_max: f32, ampl_max: f32) -> Self {
        let vert_bytes = load_file("src/spectrum_vert.gl").await.expect("Failed to load vertex shader");
        let frag_bytes = load_file("src/spectrum_frag.gl").await.expect("Failed to load fragment shader");
        let vert_src = String::from_utf8(vert_bytes).expect("Vertex shader is not valid UTF-8");
        let frag_src = String::from_utf8(frag_bytes).expect("Fragment shader is not valid UTF-8");

        let material = load_material(
            ShaderSource::Glsl {
                vertex: &vert_src,
                fragment: &frag_src,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("base",       UniformType::Float1),
                    UniformDesc::new("adds_count", UniformType::Int1),
                    UniformDesc::new("subs_count", UniformType::Int1),
                    UniformDesc::new("freq_min",   UniformType::Float1),
                    UniformDesc::new("freq_max",   UniformType::Float1),
                    UniformDesc::new("ampl_max",   UniformType::Float1),
                ],
                textures: vec!["gaussian_data".to_string()],
                ..Default::default()
            },
        )
        .unwrap();

        // Allocate the texture at full size upfront; update() will fill it.
        let texture = Texture2D::from_rgba8(MAX_GAUSSIANS as u16, 2, &vec![0u8; MAX_GAUSSIANS * 2 * 4]);
        texture.set_filter(FilterMode::Nearest);

        SpectrumRenderer {
            material,
            texture,
            freq_min,
            freq_max,
            ampl_max,
            base: 0.0,
            adds_count: 0,
            subs_count: 0,
        }
    }

    pub fn update(&mut self, spectrum: &Spectrum) {
        self.base       = spectrum.base;
        self.adds_count = spectrum.adds.len().min(MAX_GAUSSIANS) as i32;
        self.subs_count = spectrum.subs.len().min(MAX_GAUSSIANS) as i32;

        let freq_range = self.freq_max - self.freq_min;
        let encode = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;

        let mut pixels = vec![0u8; MAX_GAUSSIANS * 2 * 4];

        for (i, g) in spectrum.adds.iter().take(MAX_GAUSSIANS).enumerate() {
            let p = &mut pixels[i * 4..];
            p[0] = encode((g.mean - self.freq_min) / freq_range);
            p[1] = encode(g.sdev / freq_range);
            p[2] = encode(g.ampl / self.ampl_max);
            p[3] = 255;
        }
        for (i, g) in spectrum.subs.iter().take(MAX_GAUSSIANS).enumerate() {
            let p = &mut pixels[(MAX_GAUSSIANS + i) * 4..];
            p[0] = encode((g.mean - self.freq_min) / freq_range);
            p[1] = encode(g.sdev / freq_range);
            p[2] = encode(g.ampl / self.ampl_max);
            p[3] = 255;
        }

        self.texture.update(&Image {
            width:  MAX_GAUSSIANS as u16,
            height: 2,
            bytes:  pixels,
        });
    }

    pub fn draw(&self, pos: Vec2, size: Vec2) {
        self.material.set_uniform("base",       self.base);
        self.material.set_uniform("adds_count", self.adds_count);
        self.material.set_uniform("subs_count", self.subs_count);
        self.material.set_uniform("freq_min",   self.freq_min);
        self.material.set_uniform("freq_max",   self.freq_max);
        self.material.set_uniform("ampl_max",   self.ampl_max);
        self.material.set_texture("gaussian_data", self.texture.clone());

        gl_use_material(&self.material);
        draw_rectangle(pos.x - size.x / 2.0, pos.y - size.y / 2.0, size.x, size.y, WHITE);
        gl_use_default_material();
    }
}
