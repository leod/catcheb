use rand::Rng;
use slab::Slab;

use comn::GameTime;

struct Particle {
    pos: comn::Point,
    vel: comn::Vector,
    angle: f32,
    angle_vel: f32,
    life: GameTime,
    damping: f32,
    color: Color,
    size: f32,
}

pub struct Particles {
    particles: Slab<Particle>,
}

impl Particles {
    pub fn new() -> Self {
        Self {
            particles: Slab::new(),
        }
    }

    pub fn spawn_blood(&mut self, pos: comn::Point, bamness: f32) {
        let mut rng = rand::thread_rng();
        let num = (bamness / 2.0) as usize;

        for _ in 0..num {
            let dir = rng.gen::<f32>() * std::f32::consts::PI * 2.0;
            let speed_factor = rng.gen::<f32>();
            let speed = 50.0 + speed_factor * 500.0;
            let particle = Particle {
                pos,
                vel: speed * comn::Vector::new(dir.cos(), dir.sin()),
                angle: 0.0,
                angle_vel: rng.gen_range(-1.0, 1.0) * 120.0,
                life: 5.0 + rng.gen_range(-1.0, 1.0),
                damping: 7.0 + speed_factor * rng.gen::<f32>() * 9.0,
                color: Color {
                    r: 1.0 - rng.gen::<f32>() * 0.2,
                    g: 0.0,
                    b: 0.0,
                    //g: rng.gen::<f32>() * 0.5,
                    //b: rng.gen::<f32>() * 0.5,
                    a: 1.0,
                },
                size: rng.gen_range(7.0, 20.0),
            };
            self.particles.insert(particle);
        }
    }

    pub fn spawn_trail(
        &mut self,
        pos: comn::Point,
        angle: f32,
        angle_spread: f32,
        speed: f32,
        color: Color,
        size: f32,
        num: usize,
    ) {
        let mut rng = rand::thread_rng();

        for _ in 0..num {
            let particle_dir = angle + rng.gen_range(-1.0, 1.0) * angle_spread;
            let particle_speed = speed + rng.gen_range(0.0, 50.0);
            let particle = Particle {
                pos,
                vel: particle_speed * comn::Vector::new(particle_dir.cos(), particle_dir.sin()),
                angle: 0.0,
                angle_vel: rng.gen_range(-1.0, 1.0) * 120.0,
                life: 0.3,
                damping: 15.0 + rng.gen_range(0.0, 5.0),
                color: Color {
                    r: 0.3,
                    g: 0.5,
                    b: rng.gen_range(0.8, 1.0),
                    a: 1.0,
                },
                size,
            };
            self.particles.insert(particle);
        }
    }

    pub fn update(&mut self, dt: GameTime) {
        for (_, particle) in self.particles.iter_mut() {
            particle.pos += particle.vel * dt;
            particle.vel -= particle.damping * particle.vel * dt;
            if particle.vel.norm() < 0.01 {
                particle.vel = comn::Vector::zeros();
            }

            particle.angle += particle.angle_vel * dt;
            particle.angle_vel -= 2.0 * particle.damping * particle.angle_vel * dt;
            if particle.angle_vel.abs() < 0.01 {
                particle.angle_vel = 0.0;
            }

            particle.life -= dt;
        }

        self.particles.retain(|_, particle| particle.life >= 0.0);
    }

    pub fn render(&self, gfx: &mut Graphics, camera_transform: Transform) {
        for (_, particle) in self.particles.iter() {
            let rect = Rectangle::new(Vector::new(-0.5, -0.5), Vector::new(1.0, 1.0));
            let pos: mint::Vector2<f32> = particle.pos.coords.into();
            let transform = Transform::rotate(particle.angle.to_degrees())
                .then(Transform::scale(Vector::new(particle.size, particle.size)))
                .then(Transform::translate(pos.into()))
                .then(camera_transform);

            let alpha = pareen::constant(1.0)
                .seq_ease_out(
                    -0.15,
                    pareen::easer::functions::Sine,
                    0.15,
                    pareen::constant(0.0),
                )
                .eval(-particle.life);

            gfx.set_transform(transform);
            gfx.fill_rect(
                &rect,
                Color {
                    a: alpha,
                    ..particle.color
                },
            );
        }

        gfx.set_transform(Transform::IDENTITY);
    }
}
