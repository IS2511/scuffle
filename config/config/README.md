# ⚙️ Config

Someone once said

> Config Solutions Absolutely Suck

This crate tries to solve this problem and make it suck less.

Comes with built-in support for parsing cli arguments, environment variables
and files, without requiring you to write more than just the definition of your
configuration structs.

Another goal of this crate is to be extensible and allow you to write your own
sources. You could, for example, implement a database or a remote service as
a config source.

See [docs](https://docs.rs/CRATES_IO_PACKAGE_NAME_HERE/) for more information.

## 🚀 Usage

Since this crate is built on top of [serde](https://crates.io/crates/serde),
please add serde as a dependency to your `Cargo.toml`.

`cargo add serde --features derive`

After that, you can use the `#[derive(Config)]` macro.
All structs implementing `Config` are also required to implement
[`serde::Deserialize`](https://docs.rs/serde/1/serde/trait.Deserialize.html).

```rust no_run
use config::sources;

#[derive(config::Config, serde::Deserialize)]
struct MyConfig {
    // ...
}

fn main() -> Result<(), config::ConfigError> {
    let mut builder = config::ConfigBuilder::new();
    // From CLI arguments
    builder.add_source(sources::CliSource::new()?);
    // From environment variables
    builder.add_source(sources::EnvSource::with_prefix("TEST")?);
    // From config file
    builder.add_source(sources::FileSource::with_path("config.toml")?);

    // Build the final configuration
    let config: MyConfig = builder.build()?;

    // ...

    Ok(())
}
```

See [docs](https://docs.rs/CRATES_IO_PACKAGE_NAME_HERE/) for more information.

### Example

See the [examples](./examples) folder for examples.
