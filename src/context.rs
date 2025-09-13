use crate::bindings;
use crate::{Agent, HsaError, MemoryRegion, Queue, Result};

pub struct HsaContext {
    pub agent: Agent,
    pub queue: Option<Queue>,
    pub kernarg_region: Option<MemoryRegion>,
    pub fine_grained_region: Option<MemoryRegion>,
    pub coarse_grained_region: Option<MemoryRegion>,
}

impl HsaContext {
    pub fn new() -> Result<Self> {
        crate::init()?;

        let agent = Agent::find_gpu()?;
        let regions = agent.iterate_memory_regions()?;

        let mut kernarg_region = None;
        let mut fine_grained_region = None;
        let mut coarse_grained_region = None;

        for region in regions {
            match region.segment()? {
                bindings::hsa_region_segment_t_HSA_REGION_SEGMENT_KERNARG => {
                    kernarg_region = Some(region);
                }
                bindings::hsa_region_segment_t_HSA_REGION_SEGMENT_GLOBAL => {
                    let flags = region.global_flags()?;
                    if flags
                        & bindings::hsa_region_global_flag_t_HSA_REGION_GLOBAL_FLAG_FINE_GRAINED
                        != 0
                    {
                        fine_grained_region = Some(region);
                    } else {
                        coarse_grained_region = Some(region);
                    }
                }
                _ => {}
            }
        }

        let coarse_grained_region = coarse_grained_region.ok_or(HsaError::MemoryRegionNotFound)?;
        let fine_grained_region = fine_grained_region.ok_or(HsaError::MemoryRegionNotFound)?;

        let queue = Queue::create(&agent, 1024)?;

        Ok(Self {
            agent,
            queue: Some(queue),
            kernarg_region,
            fine_grained_region: Some(fine_grained_region),
            coarse_grained_region: Some(coarse_grained_region),
        })
    }
}

impl Drop for HsaContext {
    fn drop(&mut self) {
        self.queue.take();
        let _ = crate::shutdown();
    }
}
