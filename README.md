# plugged
Wasm plugins made easy.

## Getting started

### Using plugins
```rust
use plugged::Plugin;

let plugin = Plugin::new("./path/to/your/plugin.wasm")?;
let func = plugin.function::<(i32, i32), i32>("add")?;
let result = func((42, 1))?;
println!("42 + 1 = {result}");
```

### Writing plugins
`./src/lib.rs`
```rust
#[no_mangle]
pub fn add(left: i32, right: i32) -> i32 {
    left + right
}
```
`./Cargo.toml`
```toml
# ...
[lib]
crate-type = ["cdylib"]
# ...
```
`./.cargo/config.toml`
```toml
[build]
target = "wasm32-unknown-unknown"
```

