use crate::bindings;
use crate::{HsaError, MemoryRegion, Result};
use std::os::raw::c_void;

#[derive(Debug, Clone, Copy)]
pub struct Agent {
    pub(crate) handle: bindings::hsa_agent_t,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Cpu,
    Gpu,
    Dsp,
}

impl Agent {
    pub fn find_gpu() -> Result<Self> {
        let mut agent_handle = bindings::hsa_agent_t { handle: 0 };

        unsafe {
            let status = bindings::hsa_iterate_agents(
                Some(find_gpu_callback),
                &mut agent_handle as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS
                && status != bindings::hsa_status_t_HSA_STATUS_INFO_BREAK
            {
                return Err(HsaError::AgentNotFound);
            }
        }

        if agent_handle.handle == 0 {
            return Err(HsaError::AgentNotFound);
        }

        Ok(Agent {
            handle: agent_handle,
        })
    }

    pub fn device_type(&self) -> Result<DeviceType> {
        let mut device_type = bindings::hsa_device_type_t_HSA_DEVICE_TYPE_CPU;
        unsafe {
            let status = bindings::hsa_agent_get_info(
                self.handle,
                bindings::hsa_agent_info_t_HSA_AGENT_INFO_DEVICE,
                &mut device_type as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status(status));
            }
        }

        Ok(match device_type {
            bindings::hsa_device_type_t_HSA_DEVICE_TYPE_CPU => DeviceType::Cpu,
            bindings::hsa_device_type_t_HSA_DEVICE_TYPE_GPU => DeviceType::Gpu,
            bindings::hsa_device_type_t_HSA_DEVICE_TYPE_DSP => DeviceType::Dsp,
            _ => return Err(HsaError::InvalidArgument),
        })
    }

    pub fn iterate_memory_regions(&self) -> Result<Vec<MemoryRegion>> {
        let mut regions = Vec::new();

        unsafe {
            let status = bindings::hsa_agent_iterate_regions(
                self.handle,
                Some(collect_regions_callback),
                &mut regions as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status(status));
            }
        }

        Ok(regions)
    }
}

unsafe extern "C" fn find_gpu_callback(
    agent: bindings::hsa_agent_t,
    data: *mut c_void,
) -> bindings::hsa_status_t {
    let mut device_type = bindings::hsa_device_type_t_HSA_DEVICE_TYPE_CPU;
    let status = unsafe {
        bindings::hsa_agent_get_info(
            agent,
            bindings::hsa_agent_info_t_HSA_AGENT_INFO_DEVICE,
            &mut device_type as *mut _ as *mut c_void,
        )
    };

    if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
        return status;
    }

    if device_type == bindings::hsa_device_type_t_HSA_DEVICE_TYPE_GPU {
        let agent_ptr = data as *mut bindings::hsa_agent_t;
        unsafe { *agent_ptr = agent };
        return bindings::hsa_status_t_HSA_STATUS_INFO_BREAK;
    }

    bindings::hsa_status_t_HSA_STATUS_SUCCESS
}

unsafe extern "C" fn collect_regions_callback(
    region: bindings::hsa_region_t,
    data: *mut c_void,
) -> bindings::hsa_status_t {
    let regions = unsafe { &mut *(data as *mut Vec<MemoryRegion>) };
    regions.push(MemoryRegion { handle: region });
    bindings::hsa_status_t_HSA_STATUS_SUCCESS
}
