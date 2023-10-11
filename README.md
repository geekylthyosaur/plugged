# plugged
Wasm plugins made easy.

## Getting started

### Example of usage
```rust
use plugged::Plugin;

let plugin = Plugin::new("./path/to/your/plugin.wasm")?;
let func = plugin.function::<(i32, i32), i32>("add")?;
let result = func.call((42, 1))?;
println!("42 + 1 = {result}");
```

