use crate::config::{DEFAULT_PREFETCH_AMOUNT, MINIMUM_LOG_DELTA_TIME, USE_SECOND_BUCKET};
use crate::io::concurrent::temp_reads::creads_utils::CompressedReadsBucketHelper;
use crate::pipeline_common::kmers_transform::processor::KmersTransformProcessor;
use crate::pipeline_common::kmers_transform::reads_buffer::ReadsBuffer;
use crate::pipeline_common::kmers_transform::resplitter::KmersTransformResplitter;
use crate::pipeline_common::kmers_transform::{
    KmersTransformContext, KmersTransformExecutorFactory, KmersTransformPreprocessor,
};
use crate::pipeline_common::minimizer_bucketing::counters_analyzer::BucketCounter;
use crate::utils::compressed_read::CompressedReadIndipendent;
use crate::KEEP_FILES;
use itertools::Itertools;
use parallel_processor::buckets::readers::async_binary_reader::AsyncBinaryReader;
use parallel_processor::execution_manager::executor::{Executor, ExecutorType};
use parallel_processor::execution_manager::executor_address::ExecutorAddress;
use parallel_processor::execution_manager::objects_pool::PoolObjectTrait;
use parallel_processor::execution_manager::packet::{Packet, PacketTrait};
use parallel_processor::memory_fs::RemoveFileMode;
use parallel_processor::phase_times_monitor::PHASES_TIMES_MONITOR;
use replace_with::replace_with_or_abort;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct KmersTransformReader<F: KmersTransformExecutorFactory> {
    context: Option<Arc<KmersTransformContext<F>>>,
    second_buckets_count_log: usize,
    buffers: Vec<Packet<ReadsBuffer<F::AssociatedExtraData>>>,
    addresses: Vec<ExecutorAddress>,
    preprocessor: Option<F::PreprocessorType>,
    async_reader: Option<AsyncBinaryReader>,
    _phantom: PhantomData<F>,
}

pub struct InputBucketDesc {
    pub(crate) path: PathBuf,
    pub(crate) sub_bucket_counters: Vec<BucketCounter>,
    pub(crate) resplitted: bool,
}

impl PoolObjectTrait for InputBucketDesc {
    type InitData = ();

    fn allocate_new(init_data: &Self::InitData) -> Self {
        Self {
            path: PathBuf::new(),
            sub_bucket_counters: Vec::new(),
            resplitted: false,
        }
    }

    fn reset(&mut self) {
        self.resplitted = false;
        self.sub_bucket_counters.clear();
    }
}
impl PacketTrait for InputBucketDesc {}

impl<F: KmersTransformExecutorFactory> PoolObjectTrait for KmersTransformReader<F> {
    type InitData = ();

    fn allocate_new(_init_data: &Self::InitData) -> Self {
        Self {
            context: None,
            second_buckets_count_log: 0,
            buffers: vec![],
            addresses: vec![],
            preprocessor: None,
            async_reader: None,
            _phantom: Default::default(),
        }
    }

    fn reset(&mut self) {
        self.context.take();
        self.buffers.clear();
        self.addresses.clear();
        self.preprocessor.take();
        self.async_reader.take();
    }
}

impl<F: KmersTransformExecutorFactory> Executor for KmersTransformReader<F> {
    const EXECUTOR_TYPE: ExecutorType = ExecutorType::NeedsInitPacket;

    const BASE_PRIORITY: u64 = 0;
    const PACKET_PRIORITY_MULTIPLIER: u64 = 2;
    const STRICT_POOL_ALLOC: bool = true;

    type InputPacket = InputBucketDesc;
    type OutputPacket = ReadsBuffer<F::AssociatedExtraData>;
    type GlobalParams = KmersTransformContext<F>;
    type MemoryParams = ();
    type BuildParams = (
        Arc<KmersTransformContext<F>>,
        AsyncBinaryReader,
        usize,
        Vec<ExecutorAddress>,
    );

    fn allocate_new_group<D: FnOnce(Vec<ExecutorAddress>)>(
        global_params: Arc<Self::GlobalParams>,
        _memory_params: Option<Self::MemoryParams>,
        common_packet: Option<Packet<Self::InputPacket>>,
        executors_initializer: D,
    ) -> (Self::BuildParams, usize) {
        let file = common_packet.unwrap();

        // FIXME: Choose the right number of executors depending on size
        let second_buckets_count_log = global_params.max_second_buckets_count_log2;

        // TODO: Enable resplitting
        let addresses: Vec<_> = (0..(1 << second_buckets_count_log))
            .map(|i| {
                if !file.resplitted
                    && i < file.sub_bucket_counters.len()
                    && file.sub_bucket_counters[i].is_outlier
                {
                    println!(
                        "Sub-bucket {} is an outlier with size {}!",
                        i, file.sub_bucket_counters[i].count
                    );
                    KmersTransformResplitter::<F>::generate_new_address()
                } else {
                    KmersTransformProcessor::<F>::generate_new_address()
                }
            })
            .collect();

        let max_concurrency = 4; // TODO: Optimize for bucket size

        executors_initializer(addresses.clone());

        (
            (
                global_params,
                AsyncBinaryReader::new(
                    &file.path,
                    true,
                    RemoveFileMode::Remove {
                        remove_fs: !KEEP_FILES.load(Ordering::Relaxed),
                    },
                    DEFAULT_PREFETCH_AMOUNT,
                ),
                second_buckets_count_log,
                addresses,
            ),
            max_concurrency,
        )
    }

    fn required_pool_items(&self) -> u64 {
        1
    }

    fn pre_execute<
        P: FnMut() -> Packet<Self::OutputPacket>,
        S: FnMut(ExecutorAddress, Packet<Self::OutputPacket>),
    >(
        &mut self,
        reinit_params: Self::BuildParams,
        mut packet_alloc: P,
        mut packet_send: S,
    ) {
        self.context = Some(reinit_params.0);

        if reinit_params.1.is_finished() {
            return;
        }

        self.second_buckets_count_log = reinit_params.2;
        let second_buckets_count = (1 << self.second_buckets_count_log);

        self.buffers
            .extend((0..second_buckets_count).map(|_| packet_alloc()));

        self.addresses = reinit_params.3.clone();

        self.preprocessor = Some(F::new_preprocessor(
            &self.context.as_ref().unwrap().global_extra_data,
        ));

        self.async_reader = Some(reinit_params.1);

        let async_reader_thread = self.context.as_ref().unwrap().async_readers.get();
        let preprocessor = self.preprocessor.as_ref().unwrap();
        let global_extra_data = &self.context.as_ref().unwrap().global_extra_data;

        self.async_reader
            .as_ref()
            .unwrap()
            .decode_all_bucket_items::<CompressedReadsBucketHelper<
                F::AssociatedExtraData,
                F::FLAGS_COUNT,
                { USE_SECOND_BUCKET },
                false,
            >, _>(async_reader_thread.clone(), Vec::new(), |read_info| {
                let bucket = preprocessor.get_sequence_bucket(global_extra_data, &read_info)
                    as usize
                    % (1 << self.second_buckets_count_log);

                let (flags, _second_bucket, extra_data, read) = read_info;

                let ind_read = CompressedReadIndipendent::from_read(
                    &read,
                    &mut self.buffers[bucket].reads_buffer,
                );
                self.buffers[bucket]
                    .reads
                    .push((flags, extra_data, ind_read));
                if self.buffers[bucket].reads.len() == self.buffers[bucket].reads.capacity() {
                    replace_with_or_abort(&mut self.buffers[bucket], |buffer| {
                        packet_send(self.addresses[bucket].clone(), buffer);
                        packet_alloc()
                    });
                }
            });
        for (packet, address) in self.buffers.drain(..).zip(self.addresses.drain(..)) {
            if packet.reads.len() > 0 {
                packet_send(address, packet);
            }
        }
    }

    fn execute<
        P: FnMut() -> Packet<Self::OutputPacket>,
        S: FnMut(ExecutorAddress, Packet<Self::OutputPacket>),
    >(
        &mut self,
        _input_packet: Packet<Self::InputPacket>,
        _packet_alloc: P,
        _packet_send: S,
    ) {
        panic!("Multiple packet processing not supported!");
    }

    fn finalize<S: FnMut(ExecutorAddress, Packet<Self::OutputPacket>)>(&mut self, _packet_send: S) {
        assert_eq!(self.buffers.len(), 0);
    }

    fn is_finished(&self) -> bool {
        self.context.is_some()
    }

    fn get_total_memory(&self) -> u64 {
        0
    }

    fn get_current_memory_params(&self) -> Self::MemoryParams {
        ()
    }
}