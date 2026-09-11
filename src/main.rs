use fractal_explorer::Sprite;
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

    let sprite = Sprite::solid(128, 128, 0x00a8ff);
    let mut framebuffer = vec![0x101820; WIDTH * HEIGHT];
    let x = (WIDTH - sprite.width()) as isize / 2;
    let y = (HEIGHT - sprite.height()) as isize / 2;
    sprite.draw_into(&mut framebuffer, WIDTH, x, y);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update_with_buffer(&framebuffer, WIDTH, HEIGHT)?;
    }

    Ok(())
}
