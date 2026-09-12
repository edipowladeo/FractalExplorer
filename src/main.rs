use fractal_explorer::Mandelbrot;
use minifb::{Key, Window, WindowOptions};

const WIDTH: usize = 640;
const HEIGHT: usize = 480;

fn main() -> Result<(), minifb::Error> {
    let mut window = Window::new(
        "FractalExplorer - Sprite",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )?;

    let fractal = Mandelbrot::new(256);
    let sprite = fractal.render(WIDTH, HEIGHT, 4.0);
    let mut framebuffer = vec![0x101820; WIDTH * HEIGHT];
    sprite.draw_into(&mut framebuffer, WIDTH, 0, 0);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update_with_buffer(&framebuffer, WIDTH, HEIGHT)?;
    }

    Ok(())
}
