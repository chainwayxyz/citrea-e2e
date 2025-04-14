use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ThrottleConfig {
    pub cpu: Option<CpuThrottle>,
    pub memory: Option<MemoryThrottle>,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            cpu: None,
            memory: None,
        }
    }
}

impl ThrottleConfig {
    /// 1 CPU and 1024 mb
    pub fn constrained() -> Self {
        Self {
            cpu: Some(CpuThrottle { cpus: 1.0 }),
            memory: Some(MemoryThrottle::mb(1024)),
        }
    }

    /// 0.1 CPU and 64mb
    pub fn flakiest() -> ThrottleConfig {
        ThrottleConfig {
            cpu: Some(CpuThrottle { cpus: 0.1 }),
            memory: Some(MemoryThrottle::mb(64)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct CpuThrottle {
    pub cpus: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MemoryThrottle {
    pub limit: u64,
}

impl MemoryThrottle {
    fn mb(v: u64) -> Self {
        Self {
            limit: v * 1024 * 1024,
        }
    }
}
