use crate::execution_manager::executor_address::ExecutorAddress;
use crate::execution_manager::manager::{ExecutionManager, ExecutionManagerTrait};
use crate::execution_manager::objects_pool::{ObjectsPool, PoolObject, PoolObjectTrait};
use crate::execution_manager::packet::Packet;
use crate::execution_manager::packet::PacketTrait;
use crate::execution_manager::work_scheduler::ExecutorDropper;
use parking_lot::RwLock;
use std::any::Any;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Eq, PartialEq)]
pub enum ExecutorType {
    SimplePacketsProcessing,
    NeedsInitPacket,
}

static EXECUTOR_GLOBAL_ID: AtomicU64 = AtomicU64::new(0);

pub trait Executor: PoolObjectTrait<InitData = ()> + Sync + Send {
    const EXECUTOR_TYPE: ExecutorType;

    const BASE_PRIORITY: u64;
    const PACKET_PRIORITY_MULTIPLIER: u64;
    const STRICT_POOL_ALLOC: bool;

    type InputPacket: Send + Sync;
    type OutputPacket: Send + Sync + PacketTrait;
    type GlobalParams: Send + Sync;
    type MemoryParams: Send + Sync;
    type BuildParams: Send + Sync + Clone;

    fn generate_new_address() -> ExecutorAddress {
        let exec = ExecutorAddress {
            executor_keeper: Arc::new(ExecutorDropper::new()),
            executor_type_id: std::any::TypeId::of::<Self>(),
            executor_internal_id: EXECUTOR_GLOBAL_ID.fetch_add(1, Ordering::Relaxed),
        };
        exec
    }

    fn allocate_new_group<D: FnOnce(Vec<ExecutorAddress>)>(
        global_params: Arc<Self::GlobalParams>,
        memory_params: Option<Self::MemoryParams>,
        common_packet: Option<Packet<Self::InputPacket>>,
        executors_initializer: D,
    ) -> (Self::BuildParams, usize);

    fn required_pool_items(&self) -> u64;

    fn pre_execute<
        P: FnMut() -> Packet<Self::OutputPacket>,
        S: FnMut(ExecutorAddress, Packet<Self::OutputPacket>),
    >(
        &mut self,
        reinit_params: Self::BuildParams,
        packet_alloc: P,
        packet_send: S,
    );

    fn execute<
        P: FnMut() -> Packet<Self::OutputPacket>,
        S: FnMut(ExecutorAddress, Packet<Self::OutputPacket>),
    >(
        &mut self,
        input_packet: Packet<Self::InputPacket>,
        packet_alloc: P,
        packet_send: S,
    );

    fn finalize<S: FnMut(ExecutorAddress, Packet<Self::OutputPacket>)>(&mut self, packet_send: S);

    fn is_finished(&self) -> bool;

    fn get_total_memory(&self) -> u64;
    fn get_current_memory_params(&self) -> Self::MemoryParams;
}