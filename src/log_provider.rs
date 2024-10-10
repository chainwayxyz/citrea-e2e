use std::path::PathBuf;

use crate::node::NodeKind;

pub trait LogPathProvider {
    fn kind() -> NodeKind;
    fn log_path(&self) -> PathBuf;
    fn stderr_path(&self) -> PathBuf;
    fn as_erased(&self) -> &dyn LogPathProviderErased
    where
        Self: Sized,
    {
        self
    }
}

pub trait LogPathProviderErased {
    fn kind(&self) -> NodeKind;
    fn log_path(&self) -> PathBuf;
    fn stderr_path(&self) -> PathBuf;
}

impl<T: LogPathProvider> LogPathProviderErased for T {
    fn kind(&self) -> NodeKind {
        T::kind()
    }

    fn log_path(&self) -> PathBuf {
        LogPathProvider::log_path(self)
    }

    fn stderr_path(&self) -> PathBuf {
        LogPathProvider::stderr_path(self)
    }
}
