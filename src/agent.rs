use crate::bindings;
use crate::error::{log_debug, log_error, log_info};
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
    Aie,
}

impl Agent {
    pub fn find_gpu() -> Result<Self> {
        log_debug("Searching for GPU agent...");

        let mut agent_handle = bindings::hsa_agent_t { handle: 0 };

        unsafe {
            let status = bindings::hsa_iterate_agents(
                Some(find_gpu_callback),
                &mut agent_handle as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS
                && status != bindings::hsa_status_t_HSA_STATUS_INFO_BREAK
            {
                let error = HsaError::from_status_with_context(status, "Failed to iterate agents");
                log_error(&format!("Agent iteration failed: {}", error));
                return Err(error);
            }
        }

        if agent_handle.handle == 0 {
            log_error("No GPU agent found in the system");
            return Err(HsaError::AgentNotFound);
        }

        log_info(&format!(
            "Found GPU agent with handle: 0x{:x}",
            agent_handle.handle
        ));
        Ok(Agent {
            handle: agent_handle,
        })
    }

    pub fn find_all() -> Result<Vec<Self>> {
        log_debug("Finding all agents...");

        let mut agents = Vec::new();

        unsafe {
            let status = bindings::hsa_iterate_agents(
                Some(collect_all_agents_callback),
                &mut agents as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error =
                    HsaError::from_status_with_context(status, "Failed to iterate all agents");
                log_error(&format!("Agent collection failed: {}", error));
                return Err(error);
            }
        }

        log_info(&format!("Found {} agents total", agents.len()));
        Ok(agents)
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
                let error =
                    HsaError::from_status_with_context(status, "Failed to get agent device type");
                log_error(&format!("Device type query failed: {}", error));
                return Err(error);
            }
        }

        let device_type = match device_type {
            bindings::hsa_device_type_t_HSA_DEVICE_TYPE_CPU => DeviceType::Cpu,
            bindings::hsa_device_type_t_HSA_DEVICE_TYPE_GPU => DeviceType::Gpu,
            bindings::hsa_device_type_t_HSA_DEVICE_TYPE_DSP => DeviceType::Dsp,
            bindings::hsa_device_type_t_HSA_DEVICE_TYPE_AIE => DeviceType::Aie,
            _ => {
                log_error(&format!("Unknown device type: {}", device_type));
                return Err(HsaError::InvalidArgument("Unknown device type".to_string()));
            }
        };

        log_debug(&format!(
            "Agent 0x{:x} device type: {:?}",
            self.handle.handle, device_type
        ));
        Ok(device_type)
    }

    pub fn get_name(&self) -> Result<String> {
        let mut name_buffer = [0u8; 64];

        unsafe {
            let status = bindings::hsa_agent_get_info(
                self.handle,
                bindings::hsa_agent_info_t_HSA_AGENT_INFO_NAME,
                name_buffer.as_mut_ptr() as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error = HsaError::from_status_with_context(status, "Failed to get agent name");
                return Err(error);
            }
        }

        // Convert to string, finding the null terminator
        let name_end = name_buffer
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(name_buffer.len());
        let name = String::from_utf8_lossy(&name_buffer[..name_end]).to_string();

        log_debug(&format!(
            "Agent 0x{:x} name: '{}'",
            self.handle.handle, name
        ));
        Ok(name)
    }

    pub fn get_vendor_name(&self) -> Result<String> {
        let mut vendor_buffer = [0u8; 64];

        unsafe {
            let status = bindings::hsa_agent_get_info(
                self.handle,
                bindings::hsa_agent_info_t_HSA_AGENT_INFO_VENDOR_NAME,
                vendor_buffer.as_mut_ptr() as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error =
                    HsaError::from_status_with_context(status, "Failed to get agent vendor name");
                return Err(error);
            }
        }

        let vendor_end = vendor_buffer
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(vendor_buffer.len());
        let vendor = String::from_utf8_lossy(&vendor_buffer[..vendor_end]).to_string();

        log_debug(&format!(
            "Agent 0x{:x} vendor: '{}'",
            self.handle.handle, vendor
        ));
        Ok(vendor)
    }

    pub fn supports_kernel_dispatch(&self) -> Result<bool> {
        let mut feature = 0u32;

        unsafe {
            let status = bindings::hsa_agent_get_info(
                self.handle,
                bindings::hsa_agent_info_t_HSA_AGENT_INFO_FEATURE,
                &mut feature as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error =
                    HsaError::from_status_with_context(status, "Failed to get agent features");
                return Err(error);
            }
        }

        let supports =
            (feature & bindings::hsa_agent_feature_t_HSA_AGENT_FEATURE_KERNEL_DISPATCH) != 0;
        log_debug(&format!(
            "Agent 0x{:x} supports kernel dispatch: {}",
            self.handle.handle, supports
        ));
        Ok(supports)
    }

    pub fn get_queue_max_size(&self) -> Result<u32> {
        let mut max_size = 0u32;

        unsafe {
            let status = bindings::hsa_agent_get_info(
                self.handle,
                bindings::hsa_agent_info_t_HSA_AGENT_INFO_QUEUE_MAX_SIZE,
                &mut max_size as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error =
                    HsaError::from_status_with_context(status, "Failed to get queue max size");
                return Err(error);
            }
        }

        log_debug(&format!(
            "Agent 0x{:x} queue max size: {}",
            self.handle.handle, max_size
        ));
        Ok(max_size)
    }

    pub fn get_queue_min_size(&self) -> Result<u32> {
        let mut min_size = 0u32;

        unsafe {
            let status = bindings::hsa_agent_get_info(
                self.handle,
                bindings::hsa_agent_info_t_HSA_AGENT_INFO_QUEUE_MIN_SIZE,
                &mut min_size as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error =
                    HsaError::from_status_with_context(status, "Failed to get queue min size");
                return Err(error);
            }
        }

        log_debug(&format!(
            "Agent 0x{:x} queue min size: {}",
            self.handle.handle, min_size
        ));
        Ok(min_size)
    }

    pub fn iterate_memory_regions(&self) -> Result<Vec<MemoryRegion>> {
        log_debug(&format!(
            "Iterating memory regions for agent 0x{:x}",
            self.handle.handle
        ));

        let mut regions: Vec<MemoryRegion> = Vec::new();

        unsafe {
            let status = bindings::hsa_agent_iterate_regions(
                self.handle,
                Some(collect_regions_callback),
                &mut regions as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error =
                    HsaError::from_status_with_context(status, "Failed to iterate memory regions");
                log_error(&format!("Memory region iteration failed: {}", error));
                return Err(error);
            }
        }

        log_debug(&format!(
            "Found {} memory regions for agent 0x{:x}",
            regions.len(),
            self.handle.handle
        ));

        // Log details about each region
        for (i, region) in regions.iter().enumerate() {
            if let Ok(segment) = region.segment() {
                let segment_name = match segment {
                    bindings::hsa_region_segment_t_HSA_REGION_SEGMENT_GLOBAL => "GLOBAL",
                    bindings::hsa_region_segment_t_HSA_REGION_SEGMENT_READONLY => "READONLY",
                    bindings::hsa_region_segment_t_HSA_REGION_SEGMENT_PRIVATE => "PRIVATE",
                    bindings::hsa_region_segment_t_HSA_REGION_SEGMENT_GROUP => "GROUP",
                    bindings::hsa_region_segment_t_HSA_REGION_SEGMENT_KERNARG => "KERNARG",
                    _ => "UNKNOWN",
                };

                if segment == bindings::hsa_region_segment_t_HSA_REGION_SEGMENT_GLOBAL {
                    if let Ok(flags) = region.global_flags() {
                        let mut flag_names = Vec::new();
                        if flags & bindings::hsa_region_global_flag_t_HSA_REGION_GLOBAL_FLAG_KERNARG
                            != 0
                        {
                            flag_names.push("KERNARG");
                        }
                        if flags
                            & bindings::hsa_region_global_flag_t_HSA_REGION_GLOBAL_FLAG_FINE_GRAINED
                            != 0
                        {
                            flag_names.push("FINE_GRAINED");
                        }
                        if flags & bindings::hsa_region_global_flag_t_HSA_REGION_GLOBAL_FLAG_COARSE_GRAINED != 0 {
                            flag_names.push("COARSE_GRAINED");
                        }

                        let flag_str = if flag_names.is_empty() {
                            "NONE".to_string()
                        } else {
                            flag_names.join("|")
                        };

                        log_debug(&format!(
                            "  Region {}: {} (flags: {})",
                            i, segment_name, flag_str
                        ));
                    } else {
                        log_debug(&format!("  Region {}: {}", i, segment_name));
                    }
                } else {
                    log_debug(&format!("  Region {}: {}", i, segment_name));
                }
            }
        }

        Ok(regions)
    }

    pub fn print_info(&self) -> Result<()> {
        log_info(&format!(
            "Agent Information (Handle: 0x{:x}):",
            self.handle.handle
        ));

        // Device type
        match self.device_type() {
            Ok(device_type) => log_info(&format!("  Device Type: {:?}", device_type)),
            Err(e) => log_error(&format!("  Device Type: Error - {}", e)),
        }

        // Name
        match self.get_name() {
            Ok(name) => log_info(&format!("  Name: {}", name)),
            Err(e) => log_error(&format!("  Name: Error - {}", e)),
        }

        // Vendor
        match self.get_vendor_name() {
            Ok(vendor) => log_info(&format!("  Vendor: {}", vendor)),
            Err(e) => log_error(&format!("  Vendor: Error - {}", e)),
        }

        // Kernel dispatch support
        match self.supports_kernel_dispatch() {
            Ok(supports) => log_info(&format!("  Supports Kernel Dispatch: {}", supports)),
            Err(e) => log_error(&format!("  Kernel Dispatch Support: Error - {}", e)),
        }

        // Queue sizes
        match (self.get_queue_min_size(), self.get_queue_max_size()) {
            (Ok(min), Ok(max)) => log_info(&format!("  Queue Size Range: {} - {}", min, max)),
            (Err(e), _) | (_, Err(e)) => log_error(&format!("  Queue Size Range: Error - {}", e)),
        }

        // Memory regions
        match self.iterate_memory_regions() {
            Ok(regions) => log_info(&format!("  Memory Regions: {} found", regions.len())),
            Err(e) => log_error(&format!("  Memory Regions: Error - {}", e)),
        }

        Ok(())
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
        log_error(&format!(
            "Failed to get device type for agent 0x{:x}",
            agent.handle
        ));
        return status;
    }

    if device_type == bindings::hsa_device_type_t_HSA_DEVICE_TYPE_GPU {
        let agent_ptr = data as *mut bindings::hsa_agent_t;
        unsafe { *agent_ptr = agent };
        return bindings::hsa_status_t_HSA_STATUS_INFO_BREAK;
    }

    bindings::hsa_status_t_HSA_STATUS_SUCCESS
}

unsafe extern "C" fn collect_all_agents_callback(
    agent: bindings::hsa_agent_t,
    data: *mut c_void,
) -> bindings::hsa_status_t {
    let agents = unsafe { &mut *(data as *mut Vec<Agent>) };
    agents.push(Agent { handle: agent });
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
