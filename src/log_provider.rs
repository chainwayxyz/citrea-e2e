use std::path::PathBuf;

use crate::node::NodeKind;

pub trait LogProvider {
    fn kind() -> NodeKind;
    fn log_path(&self) -> PathBuf;
    fn stderr_path(&self) -> PathBuf;
    fn as_erased(&self) -> &dyn LogProviderErased
    where
        Self: Sized,
    {
        self
    }
}

pub trait LogProviderErased {
    fn kind(&self) -> NodeKind;
    fn log_path(&self) -> PathBuf;
    fn stderr_path(&self) -> PathBuf;
}

impl<T: LogProvider> LogProviderErased for T {
    fn kind(&self) -> NodeKind {
        T::kind()
    }

    fn log_path(&self) -> PathBuf {
        LogProvider::log_path(self)
    }

    fn stderr_path(&self) -> PathBuf {
        LogProvider::stderr_path(self)
    }
}
