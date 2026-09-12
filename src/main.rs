use fractal_explorer::{Mandelbrot, Orchestrator, RenderConfig};

const WIDTH: usize = 640;
const HEIGHT: usize = 480;

fn main() -> Result<(), minifb::Error> {
    let image =
        Orchestrator::new(Mandelbrot::new(256)).render(RenderConfig::centered(WIDTH, HEIGHT, 4.0));
    fractal_explorer::renderer::run(image)
}
