use std::{cell::RefCell, ops::Deref, path::Path};

use wasmer::{imports, FunctionType, Instance, Module, Store, Value, WasmTypeList};

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error(transparent)]
    ExportError(#[from] wasmer::ExportError),
    #[error(transparent)]
    LoadError(#[from] anyhow::Error),
    #[error(transparent)]
    RuntimeError(#[from] wasmer::RuntimeError),
    #[error("Expected function signature {expected} but got {actual}")]
    TypeMismatchError {
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
            .map_err(PluginError::ExportError)?;
        let actual = f.ty(&self.store.borrow());
        let expected = FunctionType::new(Args::wasm_types(), Rets::wasm_types());
        if actual != expected {
            return Err(PluginError::TypeMismatchError { actual, expected });
        }

        Ok(Function::new(|args: Args| -> Result<Rets> {
            let store = &mut self.store.borrow_mut();
            let types = Args::wasm_types();
            let mut args = unsafe { args.into_array(store) };
            let args = args
                .as_mut()
                .iter()
                .zip(types.iter())
                .map(|(arg, ty)| unsafe { Value::from_raw(store, ty.to_owned(), arg.to_owned()) })
                .collect::<Vec<_>>();
            let result = f.call(store, &args)?;
            let result = result
                .iter()
                .map(|ret| ret.as_raw(store))
                .collect::<Vec<_>>();
            let result = unsafe { Rets::from_slice(store, &result).unwrap_unchecked() };
            Ok(result)
        }))
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
        assert!(matches!(result, Err(PluginError::TypeMismatchError { .. })));

        let result = plugin.function::<(i32, i32), i64>("add");
        assert!(matches!(result, Err(PluginError::TypeMismatchError { .. })));
        Ok(())
    }
}
