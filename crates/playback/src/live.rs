//! Lock-free queue of live (audition) note events from the UI thread to the audio callback.
//!
//! Single producer (the UI thread, through [`crate::Player`]), single consumer (the audio
//! callback). Fixed capacity, no allocation after construction, atomics only (no lock, no
//! `unsafe`). When the queue is full a push is refused and the event is dropped; the mix engine's
//! safety timeout releases a note whose release was lost.

use soundcraft_mix::LiveNote;
use soundcraft_model::TrackId;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

/// Events the queue holds.
pub(crate) const CAPACITY: usize = 256;

const ON: u32 = 1;
const OFF: u32 = 2;
const ALL_OFF: u32 = 4;

/// What travels through the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiveEvent {
    Note(LiveNote),
    /// Release every live note on every track.
    AllOff,
}

pub(crate) struct LiveQueue {
    tracks: Box<[AtomicU64]>,
    data: Box<[AtomicU32]>,
    head: AtomicUsize,
    tail: AtomicUsize,
    /// The audio callback's last verdict: a live note or its release tail is sounding.
    active: AtomicBool,
}

impl LiveQueue {
    pub(crate) fn new() -> LiveQueue {
        LiveQueue {
            tracks: (0..CAPACITY).map(|_| AtomicU64::new(0)).collect(),
            data: (0..CAPACITY).map(|_| AtomicU32::new(0)).collect(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            active: AtomicBool::new(false),
        }
    }

    /// Consumer side: record whether a live note (or its tail) is sounding.
    pub(crate) fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Relaxed);
    }

    /// Whether a live note is sounding or still waiting in the queue. Any thread.
    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed) || self.head.load(Ordering::Acquire) != self.tail.load(Ordering::Acquire)
    }

    /// Queue an event. `false` when the queue is full (the event is dropped). Producer side only.
    pub(crate) fn push(&self, ev: LiveEvent) -> bool {
        let (track, word) = match ev {
            LiveEvent::Note(n) => (n.track.0, u32::from(n.pitch) | u32::from(n.velocity) << 8 | (if n.on { ON } else { OFF }) << 16),
            LiveEvent::AllOff => (0, ALL_OFF << 16),
        };
        let head = self.head.load(Ordering::Relaxed);
        if head.wrapping_sub(self.tail.load(Ordering::Acquire)) >= CAPACITY {
            return false;
        }
        let i = head % CAPACITY;
        if let (Some(t), Some(d)) = (self.tracks.get(i), self.data.get(i)) {
            t.store(track, Ordering::Relaxed);
            d.store(word, Ordering::Relaxed);
        }
        self.head.store(head.wrapping_add(1), Ordering::Release);
        true
    }

    /// The oldest event, if any. Consumer side only.
    pub(crate) fn pop(&self) -> Option<LiveEvent> {
        let tail = self.tail.load(Ordering::Relaxed);
        if tail == self.head.load(Ordering::Acquire) {
            return None;
        }
        let i = tail % CAPACITY;
        let track = self.tracks.get(i).map_or(0, |t| t.load(Ordering::Relaxed));
        let word = self.data.get(i).map_or(0, |d| d.load(Ordering::Relaxed));
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        let flags = word >> 16;
        let (pitch, velocity) = ((word & 0xFF) as u8, ((word >> 8) & 0xFF) as u8);
        Some(if flags & ALL_OFF != 0 {
            LiveEvent::AllOff
        } else {
            LiveEvent::Note(LiveNote { track: TrackId(track), pitch, velocity, on: flags & ON != 0 })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_flag_covers_queued_events() {
        let q = LiveQueue::new();
        assert!(!q.is_active());
        assert!(q.push(note(1, true)));
        assert!(q.is_active(), "a queued event counts");
        let _ = q.pop();
        assert!(!q.is_active());
        q.set_active(true);
        assert!(q.is_active());
        q.set_active(false);
        assert!(!q.is_active());
    }

    fn note(i: u64, on: bool) -> LiveEvent {
        LiveEvent::Note(LiveNote { track: TrackId(i), pitch: (i % 128) as u8, velocity: 100, on })
    }

    #[test]
    fn events_come_out_in_order() {
        let q = LiveQueue::new();
        assert_eq!(q.pop(), None);
        for i in 0..5 {
            assert!(q.push(note(i, i % 2 == 0)));
        }
        for i in 0..5 {
            assert_eq!(q.pop(), Some(note(i, i % 2 == 0)));
        }
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn wraps_around_the_ring_many_times() {
        let q = LiveQueue::new();
        for i in 0..(CAPACITY as u64 * 5 + 7) {
            assert!(q.push(note(i, true)));
            assert!(q.push(note(i + 1, false)));
            assert_eq!(q.pop(), Some(note(i, true)));
            assert_eq!(q.pop(), Some(note(i + 1, false)));
        }
    }

    #[test]
    fn a_full_queue_refuses_without_panicking() {
        let q = LiveQueue::new();
        for i in 0..CAPACITY as u64 {
            assert!(q.push(note(i, true)));
        }
        assert!(!q.push(note(999, true)), "the 257th push is dropped");
        assert_eq!(q.pop(), Some(note(0, true)));
        assert!(q.push(note(999, true)), "room again after a pop");
        let mut n = 0;
        while q.pop().is_some() {
            n += 1;
        }
        assert_eq!(n, CAPACITY);
    }

    #[test]
    fn all_off_and_note_fields_round_trip() {
        let q = LiveQueue::new();
        assert!(q.push(LiveEvent::AllOff));
        assert!(q.push(LiveEvent::Note(LiveNote { track: TrackId(u64::MAX), pitch: 127, velocity: 127, on: true })));
        assert!(q.push(LiveEvent::Note(LiveNote { track: TrackId(7), pitch: 0, velocity: 1, on: false })));
        assert_eq!(q.pop(), Some(LiveEvent::AllOff));
        assert_eq!(q.pop(), Some(LiveEvent::Note(LiveNote { track: TrackId(u64::MAX), pitch: 127, velocity: 127, on: true })));
        assert_eq!(q.pop(), Some(LiveEvent::Note(LiveNote { track: TrackId(7), pitch: 0, velocity: 1, on: false })));
    }

    #[test]
    fn works_across_threads() {
        let q = std::sync::Arc::new(LiveQueue::new());
        let p = std::sync::Arc::clone(&q);
        let h = std::thread::spawn(move || {
            let mut sent = 0u64;
            while sent < 5000 {
                if p.push(note(sent, true)) {
                    sent += 1;
                } else {
                    std::thread::yield_now();
                }
            }
        });
        let mut next = 0u64;
        while next < 5000 {
            match q.pop() {
                Some(e) => {
                    assert_eq!(e, note(next, true));
                    next += 1;
                }
                None => std::thread::yield_now(),
            }
        }
        assert!(h.join().is_ok());
    }
}
