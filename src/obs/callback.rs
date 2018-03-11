use futures::{Async, Future, Poll};
use futures::sync::oneshot;
use std::cell::Cell;
use std::os::raw::c_void;
use std::sync::Arc;
use super::libobs;

pub trait RenderCallback {
    fn execute(&self, cx: u32, cy: u32);
}

unsafe extern "C" fn main_render_callback<T>(cb: *mut c_void, cx: u32, cy: u32)
where
    T: RenderCallback,
{
    let cb = &*(cb as *mut T);
    cb.execute(cx, cy);
}

pub unsafe fn add_main_render_callback<T>(cb: &Arc<T>)
where
    T: RenderCallback + Send + Sync,
{
    let cb = Arc::into_raw(cb.clone()) as *mut c_void;
    libobs::obs_add_main_render_callback(Some(main_render_callback::<T>), cb);
}

pub unsafe fn remove_main_render_callback<T>(cb: &Arc<T>)
where
    T: RenderCallback,
{
    let cb = (&**cb) as *const T as *mut c_void;
    libobs::obs_remove_main_render_callback(Some(main_render_callback::<T>), cb);
    // decrement reference counter
    Arc::from_raw(cb);
}

struct RunOnce<T, R, E> {
    state: Cell<Option<(T, oneshot::Sender<Result<R, E>>)>>,
}

unsafe impl<T, R, E> Sync for RunOnce<T, R, E> {}

impl<T, R, E> RenderCallback for RunOnce<T, R, E>
where
    T: FnOnce(u32, u32) -> Result<R, E>,
{
    fn execute(&self, cx: u32, cy: u32) {
        let state = self.state.take();
        if let Some((cb, channel)) = state {
            if channel.is_canceled() {
                return;
            }
            let _ = channel.send(cb(cx, cy));
        }
    }
}

pub struct RunOnceFuture<T, R, E>
where
    T: FnOnce(u32, u32) -> Result<R, E>,
{
    cb: Option<Arc<RunOnce<T, R, E>>>,
    channel: oneshot::Receiver<Result<R, E>>,
}

impl<T, R, E> Future for RunOnceFuture<T, R, E>
where
    T: FnOnce(u32, u32) -> Result<R, E>,
{
    type Item = R;
    type Error = E;

    fn poll(&mut self) -> Poll<R, E> {
        let result = match self.channel.poll() {
            Ok(Async::Ready(Ok(t))) => Ok(Async::Ready(t)),
            Ok(Async::Ready(Err(t))) => Err(t),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_) => unreachable!(),
        };
        match result {
            Ok(Async::Ready(_)) | Err(_) => {
                if let Some(cb) = self.cb.take() {
                    unsafe {
                        remove_main_render_callback(&cb);
                    }
                }
            }
            Ok(Async::NotReady) => {}
        }
        result
    }
}

impl<T, R, E> Drop for RunOnceFuture<T, R, E>
where
    T: FnOnce(u32, u32) -> Result<R, E>,
{
    fn drop(&mut self) {
        if let Some(cb) = self.cb.take() {
            unsafe {
                remove_main_render_callback(&cb);
            }
        }
    }
}

pub fn execute_main_render_callback<T, R, E>(cb: T) -> RunOnceFuture<T, R, E>
where
    T: FnOnce(u32, u32) -> Result<R, E> + Send + Sized,
    R: Send + Sized,
    E: Send + Sized,
{
    let (send, receive) = oneshot::channel();
    let cb = Arc::new(RunOnce {
        state: Cell::new(Some((cb, send))),
    });
    unsafe {
        add_main_render_callback(&cb);
    }
    RunOnceFuture {
        cb: Some(cb),
        channel: receive,
    }
}
