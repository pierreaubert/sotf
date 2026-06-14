/// Format specific render callback data.
pub mod data {

    use objc2_core_audio_types::AudioBuffer;
    use objc2_core_audio_types::AudioBufferList;

    use super::super::super::Sample;
    use super::super::super::StreamFormat;
    use crate::audio_unit::audio_format::LinearPcmFlags;
    use std::marker::PhantomData;
    use std::slice;

    /// Audio data wrappers specific to the `AudioUnit`'s `AudioFormat`.
    pub trait Data {
        /// Check whether the stream format matches this type of data.
        fn does_stream_format_match(stream_format: &StreamFormat) -> bool;
        /// We must be able to construct Self from arguments given to the `input_proc`.
        /// # Safety
        /// TODO document how to use this function safely.
        unsafe fn from_input_proc_args(num_frames: u32, io_data: *mut AudioBufferList) -> Self;
    }

    /// A raw pointer to the audio data so that the user may handle it themselves.
    #[derive(Debug)]
    pub struct Raw {
        pub data: *mut AudioBufferList,
    }

    impl Data for Raw {
        fn does_stream_format_match(_: &StreamFormat) -> bool {
            true
        }
        unsafe fn from_input_proc_args(_num_frames: u32, io_data: *mut AudioBufferList) -> Self {
            Raw { data: io_data }
        }
    }

    /// An interleaved linear PCM buffer with samples of type `S`.
    pub struct Interleaved<S: 'static> {
        /// The audio buffer.
        pub buffer: &'static mut [S],
        pub channels: usize,
        sample_format: PhantomData<S>,
    }

    /// An interleaved linear PCM buffer with samples stored as plain bytes.
    pub struct InterleavedBytes<S: 'static> {
        /// The audio buffer.
        pub buffer: &'static mut [u8],
        pub channels: usize,
        sample_format: PhantomData<S>,
    }

    /// A wrapper around the pointer to the `mBuffers` array.
    pub struct NonInterleaved<S> {
        /// The list of audio buffers.
        buffers: &'static mut [AudioBuffer],
        /// The number of frames in each channel.
        frames: usize,
        sample_format: PhantomData<S>,
    }

    /// An iterator produced by a `NonInterleaved`, yielding a reference to each channel.
    pub struct Channels<'a, S: 'a> {
        buffers: slice::Iter<'a, AudioBuffer>,
        frames: usize,
        sample_format: PhantomData<S>,
    }

    /// An iterator produced by a `NonInterleaved`, yielding a mutable reference to each channel.
    pub struct ChannelsMut<'a, S: 'a> {
        buffers: slice::IterMut<'a, AudioBuffer>,
        frames: usize,
        sample_format: PhantomData<S>,
    }

    unsafe impl<S> Send for NonInterleaved<S> where S: Send {}

    impl<'a, S> Iterator for Channels<'a, S> {
        type Item = &'a [S];
        #[allow(non_snake_case)]
        fn next(&mut self) -> Option<Self::Item> {
            self.buffers.next().map(
                |&AudioBuffer {
                     mNumberChannels,
                     mData,
                     ..
                 }| {
                    let len = mNumberChannels as usize * self.frames;
                    let ptr = mData as *mut S;
                    unsafe { slice::from_raw_parts(ptr, len) }
                },
            )
        }
    }

    impl<'a, S> Iterator for ChannelsMut<'a, S> {
        type Item = &'a mut [S];
        #[allow(non_snake_case)]
        fn next(&mut self) -> Option<Self::Item> {
            self.buffers.next().map(
                |&mut AudioBuffer {
                     mNumberChannels,
                     mData,
                     ..
                 }| {
                    let len = mNumberChannels as usize * self.frames;
                    let ptr = mData as *mut S;
                    unsafe { slice::from_raw_parts_mut(ptr, len) }
                },
            )
        }
    }

    impl<S> NonInterleaved<S> {
        /// An iterator yielding a reference to each channel in the array.
        pub fn channels(&self) -> Channels<'_, S> {
            Channels {
                buffers: self.buffers.iter(),
                frames: self.frames,
                sample_format: PhantomData,
            }
        }

        /// An iterator yielding a mutable reference to each channel in the array.
        pub fn channels_mut(&mut self) -> ChannelsMut<'_, S> {
            ChannelsMut {
                buffers: self.buffers.iter_mut(),
                frames: self.frames,
                sample_format: PhantomData,
            }
        }
    }

    // Implementation for a non-interleaved linear PCM audio format.
    impl<S> Data for NonInterleaved<S>
    where
        S: Sample,
    {
        fn does_stream_format_match(stream_format: &StreamFormat) -> bool {
            stream_format
                .flags
                .contains(LinearPcmFlags::IS_NON_INTERLEAVED)
                && S::sample_format().does_match_flags(stream_format.flags)
        }

        #[allow(non_snake_case)]
        unsafe fn from_input_proc_args(frames: u32, io_data: *mut AudioBufferList) -> Self {
            let ptr = (*io_data).mBuffers.as_ptr() as *mut AudioBuffer;
            let len = (*io_data).mNumberBuffers as usize;
            let buffers = slice::from_raw_parts_mut(ptr, len);
            NonInterleaved {
                buffers,
                frames: frames as usize,
                sample_format: PhantomData,
            }
        }
    }

    // Implementation for an interleaved linear PCM audio format.
    impl<S> Data for Interleaved<S>
    where
        S: Sample,
    {
        fn does_stream_format_match(stream_format: &StreamFormat) -> bool {
            !stream_format
                .flags
                .contains(LinearPcmFlags::IS_NON_INTERLEAVED)
                && S::sample_format().does_match_flags(stream_format.flags)
        }

        #[allow(non_snake_case)]
        unsafe fn from_input_proc_args(frames: u32, io_data: *mut AudioBufferList) -> Self {
            // // We're expecting a single interleaved buffer which will be the first in the array.
            let AudioBuffer {
                mNumberChannels,
                mDataByteSize,
                mData,
            } = (*io_data).mBuffers[0];
            // // Ensure that the size of the data matches the size of the sample format
            // // multiplied by the number of frames.
            // //
            // // TODO: Return an Err instead of `panic`ing.
            let buffer_len = frames as usize * mNumberChannels as usize;
            let expected_size = ::std::mem::size_of::<S>() * buffer_len;
            assert!(mDataByteSize as usize == expected_size);

            let buffer: &mut [S] = {
                let buffer_ptr = mData as *mut S;
                slice::from_raw_parts_mut(buffer_ptr, buffer_len)
            };

            Interleaved {
                buffer,
                channels: mNumberChannels as usize,
                sample_format: PhantomData,
            }
        }
    }

    // Implementation for an interleaved linear PCM audio format using plain bytes.
    impl<S> Data for InterleavedBytes<S>
    where
        S: Sample,
    {
        fn does_stream_format_match(stream_format: &StreamFormat) -> bool {
            !stream_format
                .flags
                .contains(LinearPcmFlags::IS_NON_INTERLEAVED)
                && S::sample_format().does_match_flags(stream_format.flags)
        }

        #[allow(non_snake_case)]
        unsafe fn from_input_proc_args(frames: u32, io_data: *mut AudioBufferList) -> Self {
            // // We're expecting a single interleaved buffer which will be the first in the array.
            let AudioBuffer {
                mNumberChannels,
                mDataByteSize,
                mData,
            } = (*io_data).mBuffers[0];
            // // Ensure that the size of the data matches the size of the sample format
            // // multiplied by the number of frames.
            // //
            // // TODO: Return an Err instead of `panic`ing.
            let buffer_len = frames as usize * mNumberChannels as usize;
            let expected_size = ::std::mem::size_of::<S>() * buffer_len;
            assert!(mDataByteSize as usize == expected_size);

            let buffer: &mut [u8] = {
                let buffer_ptr = mData as *mut u8;
                slice::from_raw_parts_mut(buffer_ptr, mDataByteSize as usize)
            };

            InterleavedBytes {
                buffer,
                channels: mNumberChannels as usize,
                sample_format: PhantomData,
            }
        }
    }
}

pub mod action_flags {
    use objc2_audio_toolbox::AudioUnitRenderActionFlags;

    use std::fmt;

    bitflags! {
        pub struct ActionFlags: u32 {
            /// Called on a render notification Proc, which is called either before or after the
            /// render operation of the audio unit. If this flag is set, the proc is being called
            /// before the render operation is performed.
            ///
            /// **Available** in OS X v10.0 and later.
            const PRE_RENDER = AudioUnitRenderActionFlags::UnitRenderAction_PreRender.0;
            /// Called on a render notification Proc, which is called either before or after the
            /// render operation of the audio unit. If this flag is set, the proc is being called
            /// after the render operation is completed.
            ///
            /// **Available** in OS X v10.0 and later.
            const POST_RENDER = AudioUnitRenderActionFlags::UnitRenderAction_PostRender.0;
            /// This flag can be set in a render input callback (or in the audio unit's render
            /// operation itself) and is used to indicate that the render buffer contains only
            /// silence. It can then be used by the caller as a hint to whether the buffer needs to
            /// be processed or not.
            ///
            /// **Available** in OS X v10.2 and later.
            const OUTPUT_IS_SILENCE = AudioUnitRenderActionFlags::UnitRenderAction_OutputIsSilence.0;
            /// This is used with offline audio units (of type 'auol'). It is used when an offline
            /// unit is being preflighted, which is performed prior to when the actual offline
            /// rendering actions are performed. It is used for those cases where the offline
            /// process needs it (for example, with an offline unit that normalizes an audio file,
            /// it needs to see all of the audio data first before it can perform its
            /// normalization).
            ///
            /// **Available** in OS X v10.3 and later.
            const OFFLINE_PREFLIGHT = AudioUnitRenderActionFlags::OfflineUnitRenderAction_Preflight.0;
            /// Once an offline unit has been successfully preflighted, it is then put into its
            /// render mode. This flag is set to indicate to the audio unit that it is now in that
            /// state and that it should perform processing on the input data.
            ///
            /// **Available** in OS X v10.3 and later.
            const OFFLINE_RENDER = AudioUnitRenderActionFlags::OfflineUnitRenderAction_Render.0;
            /// This flag is set when an offline unit has completed either its preflight or
            /// performed render operation.
            ///
            /// **Available** in OS X v10.3 and later.
            const OFFLINE_COMPLETE = AudioUnitRenderActionFlags::OfflineUnitRenderAction_Complete.0;
            /// If this flag is set on the post-render call an error was returned by the audio
            /// unit's render operation. In this case, the error can be retrieved through the
            /// `lastRenderError` property and the audio data in `ioData` handed to the post-render
            /// notification will be invalid.
            ///
            /// **Available** in OS X v10.5 and later.
            const POST_RENDER_ERROR = AudioUnitRenderActionFlags::UnitRenderAction_PostRenderError.0;
            /// If this flag is set, then checks that are done on the arguments provided to render
            /// are not performed. This can be useful to use to save computation time in situations
            /// where you are sure you are providing the correct arguments and structures to the
            /// various render calls.
            ///
            /// **Available** in OS X v10.7 and later.
            const DO_NOT_CHECK_RENDER_ARGS = AudioUnitRenderActionFlags::UnitRenderAction_DoNotCheckRenderArgs.0;
        }
    }

    /// A safe handle around the `AudioUnitRenderActionFlags` pointer provided by the render
    /// callback.
    ///
    /// This type lets a callback provide various hints to the audio unit.
    ///
    /// For example: if there is no audio to process, we can insert the `OUTPUT_IS_SILENCE` flag to
    /// indicate to the audio unit that the buffer does not need to be processed.
    pub struct Handle {
        ptr: *mut AudioUnitRenderActionFlags,
    }

    impl fmt::Debug for Handle {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            if self.ptr.is_null() {
                write!(f, "{:?}", self.ptr)
            } else {
                unsafe { write!(f, "{:?}", *self.ptr) }
            }
        }
    }

    impl Handle {
        /// Retrieve the current state of the `ActionFlags`.
        pub fn get(&self) -> ActionFlags {
            ActionFlags::from_bits_truncate(unsafe { *self.ptr }.0)
        }

        fn set(&mut self, flags: ActionFlags) {
            unsafe { (*self.ptr).0 = flags.bits() }
        }

        /// The raw value of the flags currently stored.
        pub fn bits(&self) -> u32 {
            self.get().bits()
        }

        /// Returns `true` if no flags are currently stored.
        pub fn is_empty(&self) -> bool {
            self.get().is_empty()
        }

        /// Returns `true` if all flags are currently stored.
        pub fn is_all(&self) -> bool {
            self.get().is_all()
        }

        /// Returns `true` if there are flags common to both `self` and `other`.
        pub fn intersects(&self, other: ActionFlags) -> bool {
            self.get().intersects(other)
        }

        /// Returns `true` if all of the flags in `other` are contained within `self`.
        pub fn contains(&self, other: ActionFlags) -> bool {
            self.get().contains(other)
        }

        /// Insert the specified flags in-place.
        pub fn insert(&mut self, other: ActionFlags) {
            let mut flags = self.get();
            flags.insert(other);
            self.set(flags);
        }

        /// Remove the specified flags in-place.
        pub fn remove(&mut self, other: ActionFlags) {
            let mut flags = self.get();
            flags.remove(other);
            self.set(flags);
        }

        /// Toggles the specified flags in-place.
        pub fn toggle(&mut self, other: ActionFlags) {
            let mut flags = self.get();
            flags.toggle(other);
            self.set(flags);
        }

        /// Wrap the given pointer with a `Handle`.
        pub fn from_ptr(ptr: *mut AudioUnitRenderActionFlags) -> Self {
            Handle { ptr }
        }
    }

    unsafe impl Send for Handle {}

    impl ::std::fmt::Display for ActionFlags {
        fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
            write!(
                f,
                "{:?}",
                match AudioUnitRenderActionFlags(self.bits()) {
                    AudioUnitRenderActionFlags::UnitRenderAction_PreRender => "PRE_RENDER",
                    AudioUnitRenderActionFlags::UnitRenderAction_PostRender => "POST_RENDER",
                    AudioUnitRenderActionFlags::UnitRenderAction_OutputIsSilence =>
                        "OUTPUT_IS_SILENCE",
                    AudioUnitRenderActionFlags::OfflineUnitRenderAction_Preflight =>
                        "OFFLINE_PREFLIGHT",
                    AudioUnitRenderActionFlags::OfflineUnitRenderAction_Render => "OFFLINE_RENDER",
                    AudioUnitRenderActionFlags::OfflineUnitRenderAction_Complete =>
                        "OFFLINE_COMPLETE",
                    AudioUnitRenderActionFlags::UnitRenderAction_PostRenderError =>
                        "POST_RENDER_ERROR",
                    AudioUnitRenderActionFlags::UnitRenderAction_DoNotCheckRenderArgs =>
                        "DO_NOT_CHECK_RENDER_ARGS",
                    _ => "<Unknown ActionFlags>",
                }
            )
        }
    }
}
