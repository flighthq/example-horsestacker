mod native_app;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    native_app::run()
}
