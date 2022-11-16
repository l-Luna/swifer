use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem::swap;
use crate::gc::ManagedMem;
use crate::heap::{GcCandidate, GcPtr, Heap};

// Mark and Sweep GC
// Traces all reachable objects, marking them; then copies all marked objects to a new heap, updating their pointers

pub struct MarkAndSweepMem<T, Ptr = *const T>
    where T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>
{
    active: Heap<T, Ptr>
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> MarkAndSweepMem<T, Ptr>{
    pub fn new(size: usize) -> Self{
        return MarkAndSweepMem{
            active: Heap::new(size)
        };
    }
}

//////////////// impls

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> ManagedMem<T, Ptr> for MarkAndSweepMem<T, Ptr>{
    fn push(&mut self, v: Box<T>) -> Option<Ptr>{
        return self.active.push(v);
    }

    fn push_with(&mut self, v: Box<T>, with: impl FnOnce(Ptr) -> Ptr) -> Option<Ptr> {
        return self.active.push_with(v, with);
    }

    fn get(&self, idx: usize) -> &T{
        return self.active.get(idx);
    }

    fn get_mut(&mut self, idx: usize) -> &mut T{
        return self.active.get_mut(idx);
    }

    fn get_by(&mut self, ptr: &Ptr) -> Option<&mut T>{
        return self.active.get_by(ptr);
    }

    fn len(&self) -> usize{
        return self.active.len();
    }

    fn contains_ptr(&self, ptr: &Ptr) -> bool {
        return self.active.contains_ptr(ptr);
    }

    fn for_each(&self, cb: impl FnMut(&T, &Ptr)){
        self.active.for_each(cb);
    }

    fn gc(&mut self, roots: Vec<&mut Ptr>, weaks: Vec<&mut Ptr>){
        // new target heap
        let mut next: Heap<T, Ptr> = Heap::new(self.active.capacity());
        // mark phase: mark every reachable object
        let mut marked: HashSet<HashWrap<T, Ptr>> = HashSet::with_capacity(5);
        for root in &roots{
            mark_reachable(&mut self.active, root, &mut marked);
        }
        // sweep phase: copy marked objects to new heap and update pointers
        let mut rel: HashMap<HashWrap<T, Ptr>, HashWrap<T, Ptr>> = HashMap::with_capacity(marked.len());
        for i in (0..self.active.len()).rev(){
            let (obj, old_ptr): (Box<T>, Ptr) = self.active.take(i);
            if marked.contains(&HashWrap::new(old_ptr.clone())){
                match next.push_with(obj, |mut x| {x.copy_meta(&old_ptr); x}){
                    Some(new_ptr) => rel.insert(HashWrap::new(old_ptr), HashWrap::new(new_ptr)),
                    None => panic!("Mark and Sweep: could not allocate space in inactive heap for object")
                };
            }else{
                drop(obj);
            }
        }
        let find = |p: &Ptr| {
            rel.get(&HashWrap::new(p.clone()))
                .expect(format!("Could not find updated pointer for {:?} in table {rel:?}!", p.to_raw_ptr()).as_str())
                .ptr
                .clone()
        };
        next.for_each_mut(|o: &mut T, this: &Ptr| o.adjust_ptrs(find, this));
        // reset the active heap - should not drop anything, since everything has been moved
        self.active.reset();
        // and swap them
        swap(&mut self.active, &mut next);
        // update root pointers
        for root in roots{
            *root = find(root);
        }
        for weak in weaks{
            match rel.get(&HashWrap::new(weak.clone())) {
                None => {}
                Some(p) => *weak = p.ptr.clone()
            }
        }
    }
}

fn mark_reachable<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>>(heap: &mut Heap<T, Ptr>, root: &Ptr, marked: &mut HashSet<HashWrap<T, Ptr>>) -> usize{
    let mut count = 0;
    // unprocessed objects
    let mut stack: Vec<Ptr> = Vec::with_capacity(5);
    stack.push(root.clone());
    while let Some(current) = stack.pop(){
        if let Some(obj) = heap.get_by(&current){
            // if not already marked,
            let marker = HashWrap::new(current.clone());
            if !marked.contains(&marker){
                // mark the object
                marked.insert(marker);
                count += 1;
                // schedule every pointee for marking
                for mut ptr in obj.collect_managed_pointers(&current){
                    if Ptr::has_significant_meta(){
                        ptr = heap.to_full_ptr(&ptr);
                    }
                    stack.push(ptr);
                }
            }
        }else{
            panic!("Managed pointer {:?} not in heap!", HashWrap::new(current));
        }
    }
    return count;
}

// allow using HashMap/Debug over !Hash/!Debug Ptr

struct HashWrap<T, Ptr>
    where T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>
{
    ptr: Ptr,
    _phantom: PhantomData<T>
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> HashWrap<T, Ptr>{
    fn new(ptr: Ptr) -> Self{
        return HashWrap{
            ptr,
            _phantom: PhantomData
        };
    }
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> Hash for HashWrap<T, Ptr>{
    fn hash<H: Hasher>(&self, state: &mut H){
        self.ptr.to_raw_ptr().hash(state)
    }
}

// must be written manually due to ?Sized bound (???)
impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> PartialEq for HashWrap<T, Ptr>{
    fn eq(&self, other: &Self) -> bool{
        return self.ptr == other.ptr;
    }
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> Eq for HashWrap<T, Ptr>{}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> Debug for HashWrap<T, Ptr>{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result{
        return self.ptr.to_raw_ptr().fmt(f);
    }
}