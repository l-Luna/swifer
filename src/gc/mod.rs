use crate::heap::{GcCandidate, GcPtr, Heap};

pub mod mas;

pub trait ManagedMem<T, Ptr = *const T>
    where T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>
{
    // push, get, len, for_each, gc
    fn push(&mut self, v: Box<T>) -> Option<Ptr>;

    fn get(&self, idx: usize) -> &T;

    fn get_mut(&mut self, idx: usize) -> &mut T;

    fn get_by(&mut self, ptr: &Ptr) -> Option<&mut T>;

    fn len(&self) -> usize;

    fn contains_ptr(&self, ptr: &Ptr) -> bool;

    fn for_each(&self, cb: impl FnMut(&T));

    // TODO: is this the right representation of roots?
    fn gc(&mut self, roots: Vec<&mut Ptr>);
}

// No-GC memory, delegates directly to the (single) heap.

pub struct NoGcMem<T, Ptr = *const T>
    where T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>
{
    heap: Heap<T, Ptr>
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> NoGcMem<T, Ptr>{
    pub fn new(size: usize) -> Self{
        return NoGcMem{
            heap: Heap::new(size)
        };
    }
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> ManagedMem<T, Ptr> for NoGcMem<T, Ptr>{
    fn push(&mut self, v: Box<T>) -> Option<Ptr>{
        return self.heap.push(v);
    }

    fn get(&self, idx: usize) -> &T{
        return self.heap.get(idx);
    }

    fn get_mut(&mut self, idx: usize) -> &mut T {
        return self.heap.get_mut(idx);
    }

    fn get_by(&mut self, ptr: &Ptr) -> Option<&mut T> {
        return self.heap.get_by(ptr);
    }

    fn len(&self) -> usize{
        return self.heap.len();
    }

    fn contains_ptr(&self, ptr: &Ptr) -> bool{
        return self.heap.contains_ptr(ptr);
    }

    fn for_each(&self, cb: impl FnMut(&T)){
        self.heap.for_each(cb);
    }

    fn gc(&mut self, _roots: Vec<&mut Ptr>){
        // no-op
    }
}