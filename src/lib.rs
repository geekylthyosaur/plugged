use std::{cell::RefCell, ops::Deref, path::Path};

use wasmer::{imports, FunctionType, Instance, Module, Store, WasmTypeList};

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error(transparent)]
    Export(#[from] wasmer::ExportError),
    #[error(transparent)]
    Load(#[from] anyhow::Error),
    #[error(transparent)]
    Runtime(#[from] wasmer::RuntimeError),
    #[error("Expected function signature {expected} but got {actual}")]
    TypeMismatch {
        actual: FunctionType,
        expected: FunctionType,
    },
}

pub type Result<T> = std::result::Result<T, PluginError>;

pub struct Plugin {
    instance: Instance,
    store: RefCell<Store>,
}

impl Plugin {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = std::fs::read(path.as_ref()).map_err(anyhow::Error::from)?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self> {
        let store = RefCell::new(Store::default());
        let module = Module::new(&store.borrow(), bytes).map_err(anyhow::Error::from)?;
        let import_objects = imports! {};
        let instance = Instance::new(&mut store.borrow_mut(), &module, &import_objects)
            .map_err(anyhow::Error::from)?;

        Ok(Self { instance, store })
    }

    pub fn function<Args, Rets>(&self, name: impl AsRef<str>) -> Result<Function<Args, Rets>>
    where
        Args: WasmTypeList,
        Rets: WasmTypeList,
    {
        let f = self
            .instance
            .exports
            .get_function(name.as_ref())
            .map_err(PluginError::Export)?;

        let actual = f.ty(&self.store.borrow());
        let expected = FunctionType::new(Args::wasm_types(), Rets::wasm_types());
        if actual != expected {
            return Err(PluginError::TypeMismatch { actual, expected });
        }

        let f = |args: Args| -> Result<Rets> {
            let store = &mut self.store.borrow_mut();
            let args = unsafe { args.into_array(store) }.as_mut().into();
            let result = f.call_raw(store, args)?;
            let result = result.iter().map(|v| v.as_raw(store)).collect::<Vec<_>>();
            let result = unsafe { Rets::from_slice(store, result.as_ref()).unwrap_unchecked() };
            Ok(result)
        };

        Ok(Function::new(f))
    }
}

pub struct Function<'plugin, Args, Rets> {
    inner: Box<dyn Fn(Args) -> Result<Rets> + 'plugin>,
}

impl<'plugin, Args, Rets> Function<'plugin, Args, Rets> {
    fn new(f: impl Fn(Args) -> Result<Rets> + 'plugin) -> Self {
        Self { inner: Box::new(f) }
    }
}

impl<'plugin, Args, Rets> Deref for Function<'plugin, Args, Rets> {
    type Target = dyn Fn(Args) -> Result<Rets> + 'plugin;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wat() -> Result<()> {
        let plugin = Plugin::new("./examples/plugins/add.wat")?;
        let f = plugin.function::<(i32, i32), i32>("add")?;
        let result = f((42, 1))?;
        assert_eq!(result, 43);
        Ok(())
    }

    #[test]
    fn wasm() -> Result<()> {
        let plugin = Plugin::new(
            "./examples/plugins/add.wasm/target/wasm32-unknown-unknown/release/add.wasm",
        )?;
        let f = plugin.function::<(i32, i32), i32>("add")?;
        let result = f((42, 1))?;
        assert_eq!(result, 43);
        Ok(())
    }

    #[test]
    fn types_mismatch_fail() -> Result<()> {
        let plugin = Plugin::new("./examples/plugins/add.wat")?;

        let result = plugin.function::<(i32, i64), i32>("add");
        assert!(matches!(result, Err(PluginError::TypeMismatch { .. })));

        let result = plugin.function::<(i32, i32), i64>("add");
        assert!(matches!(result, Err(PluginError::TypeMismatch { .. })));
        Ok(())
    }
}
