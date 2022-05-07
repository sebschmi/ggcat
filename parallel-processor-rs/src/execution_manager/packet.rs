use crate::execution_manager::objects_pool::{ObjectsPool, PoolObjectTrait};
use crossbeam::channel::Sender;
use std::any::Any;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub trait PacketTrait: PoolObjectTrait + Sync + Send {}

trait PacketPoolReturnerTrait: Send + Sync {
    fn send_any(&self, packet: Box<dyn Any>);
}

impl<T: Send + Sync + 'static> PacketPoolReturnerTrait for (Sender<Box<T>>, AtomicU64) {
    fn send_any(&self, packet: Box<dyn Any>) {
        self.1.fetch_sub(1, Ordering::Relaxed);
        let _ = self.0.try_send(packet.downcast().unwrap());
    }
}

pub struct Packet<T: 'static> {
    object: ManuallyDrop<Box<T>>,
    returner: Option<Arc<dyn PacketPoolReturnerTrait>>,
}

pub struct PacketAny {
    object: ManuallyDrop<Box<dyn Any + Send + Sync>>,
    returner: Option<Arc<dyn PacketPoolReturnerTrait>>,
}

pub struct PacketsPool<T>(pub ObjectsPool<Box<T>>);

// Recursively implement the object trait for the pool, so it can be used recursively
impl<T: PacketTrait> PoolObjectTrait for PacketsPool<T> {
    type InitData = (usize, T::InitData);

    fn allocate_new((cap, init_data): &Self::InitData) -> Self {
        Self::new(*cap, init_data.clone())
    }

    fn reset(&mut self) {}
}

impl<T: PacketTrait> PacketsPool<T> {
    pub fn new(cap: usize, init_data: T::InitData) -> Self {
        Self(ObjectsPool::new(cap, init_data))
    }

    pub fn alloc_packet(&self, blocking: bool) -> Packet<T> {
        let mut object = self.0.alloc_object(blocking);

        let packet = Packet {
            object: ManuallyDrop::new(unsafe { ManuallyDrop::take(&mut object.value) }),
            returner: Some(object.returner.as_ref().unwrap().clone()),
        };

        unsafe {
            std::ptr::drop_in_place(&mut object.returner);
            std::mem::forget(object);
        }

        packet
    }

    pub fn get_available_items(&self) -> i64 {
        self.0.get_available_items()
    }

    pub fn wait_for_item_timeout(&self, timeout: Duration) {
        self.0.wait_for_item_timeout(timeout)
    }
}

impl<T: Any + Send + Sync> Packet<T> {
    pub fn new_simple(data: T) -> Self {
        Packet {
            object: ManuallyDrop::new(Box::new(data)),
            returner: None,
        }
    }

    pub fn upcast(mut self) -> PacketAny {
        let packet = PacketAny {
            object: ManuallyDrop::<Box<dyn Any + Send + Sync>>::new(unsafe {
                ManuallyDrop::<Box<T>>::take(&mut self.object)
            }),
            returner: self.returner.clone(),
        };

        unsafe {
            std::ptr::drop_in_place(&mut self.returner);
            std::mem::forget(self);
        }

        packet
    }
}

impl<T: Any + Send + Sync> Deref for Packet<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.object.as_ref()
    }
}

impl<T: Any + Send + Sync> DerefMut for Packet<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut()
    }
}

impl PacketAny {
    pub fn downcast<T: 'static>(mut self) -> Packet<T> {
        let packet = Packet {
            object: ManuallyDrop::new(unsafe {
                ManuallyDrop::take(&mut self.object).downcast().unwrap()
            }),
            returner: self.returner.clone(),
        };

        unsafe {
            std::ptr::drop_in_place(&mut self.returner);
            std::mem::forget(self);
        }

        packet
    }
}

impl<T: 'static> Drop for Packet<T> {
    fn drop(&mut self) {
        if let Some(returner) = &self.returner {
            returner.send_any(unsafe { ManuallyDrop::<Box<T>>::take(&mut self.object) });
        } else {
            unsafe { ManuallyDrop::drop(&mut self.object) }
        }
    }
}

impl Drop for PacketAny {
    fn drop(&mut self) {
        panic!("Cannot drop packet any!");
    }
}

impl PoolObjectTrait for () {
    type InitData = ();
    fn allocate_new(init_data: &Self::InitData) -> Self {
        panic!("Cannot create () type as object!");
    }

    fn reset(&mut self) {
        panic!("Cannot reset () type as object!");
    }
}
impl PacketTrait for () {}