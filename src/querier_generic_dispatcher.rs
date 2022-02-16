use crate::colors::colors_manager::ColorsManager;
use crate::hashes::cn_nthash::CanonicalNtHashIteratorFactory;
use crate::hashes::fw_nthash::ForwardNtHashIteratorFactory;
use crate::hashes::HashFunctionFactory;
use crate::querier::run_query;
use crate::utils::compute_best_m;
use crate::{HashType, QueryArgs};

fn run_querier_from_args<
    BucketingHash: HashFunctionFactory,
    MergingHash: HashFunctionFactory,
    ColorsImpl: ColorsManager,
    const BUCKETS_COUNT: usize,
>(
    args: QueryArgs,
) {
    run_query::<BucketingHash, MergingHash, ColorsImpl, BUCKETS_COUNT>(
        args.common_args.klen,
        args.common_args
            .mlen
            .unwrap_or(compute_best_m(args.common_args.klen)),
        args.step,
        args.input_graph,
        args.input_query,
        args.output_file,
        args.common_args.temp_dir,
        args.common_args.threads_count,
    );
}

pub(crate) fn dispatch_querier_hash_type<ColorsImpl: ColorsManager, const BUCKETS_COUNT: usize>(
    args: QueryArgs,
) {
    let hash_type = match args.common_args.hash_type {
        HashType::Auto => {
            if args.common_args.klen <= 64 {
                HashType::SeqHash
            } else {
                HashType::RabinKarp128
            }
        }
        x => x,
    };

    use crate::hashes::*;

    match hash_type {
        HashType::SeqHash => {
            let k = args.common_args.klen;

            if k <= 8 {
                if args.common_args.forward_only {
                    run_querier_from_args::<
                        CanonicalNtHashIteratorFactory,
                        cn_seqhash::u16::CanonicalSeqHashFactory,
                        ColorsImpl,
                        BUCKETS_COUNT,
                    >(args);
                } else {
                    run_querier_from_args::<
                        CanonicalNtHashIteratorFactory,
                        cn_seqhash::u16::CanonicalSeqHashFactory,
                        ColorsImpl,
                        BUCKETS_COUNT,
                    >(args)
                }
            } else if k <= 16 {
                if args.common_args.forward_only {
                    run_querier_from_args::<
                        ForwardNtHashIteratorFactory,
                        fw_seqhash::u32::ForwardSeqHashFactory,
                        ColorsImpl,
                        BUCKETS_COUNT,
                    >(args);
                } else {
                    run_querier_from_args::<
                        CanonicalNtHashIteratorFactory,
                        cn_seqhash::u32::CanonicalSeqHashFactory,
                        ColorsImpl,
                        BUCKETS_COUNT,
                    >(args)
                }
            } else if k <= 32 {
                if args.common_args.forward_only {
                    run_querier_from_args::<
                        ForwardNtHashIteratorFactory,
                        fw_seqhash::u64::ForwardSeqHashFactory,
                        ColorsImpl,
                        BUCKETS_COUNT,
                    >(args);
                } else {
                    run_querier_from_args::<
                        CanonicalNtHashIteratorFactory,
                        cn_seqhash::u64::CanonicalSeqHashFactory,
                        ColorsImpl,
                        BUCKETS_COUNT,
                    >(args)
                }
            } else if k <= 64 {
                if args.common_args.forward_only {
                    run_querier_from_args::<
                        ForwardNtHashIteratorFactory,
                        fw_seqhash::u128::ForwardSeqHashFactory,
                        ColorsImpl,
                        BUCKETS_COUNT,
                    >(args);
                } else {
                    run_querier_from_args::<
                        CanonicalNtHashIteratorFactory,
                        cn_seqhash::u128::CanonicalSeqHashFactory,
                        ColorsImpl,
                        BUCKETS_COUNT,
                    >(args)
                }
            } else {
                panic!("Cannot use sequence hash for k > 64!");
            }
        }
        HashType::RabinKarp32 => {
            if args.common_args.forward_only {
                run_querier_from_args::<
                    ForwardNtHashIteratorFactory,
                    fw_rkhash::u32::ForwardRabinKarpHashFactory,
                    ColorsImpl,
                    BUCKETS_COUNT,
                >(args);
            } else {
                run_querier_from_args::<
                    CanonicalNtHashIteratorFactory,
                    cn_rkhash::u32::CanonicalRabinKarpHashFactory,
                    ColorsImpl,
                    BUCKETS_COUNT,
                >(args)
            }
        }
        HashType::RabinKarp64 => {
            if args.common_args.forward_only {
                run_querier_from_args::<
                    ForwardNtHashIteratorFactory,
                    fw_rkhash::u64::ForwardRabinKarpHashFactory,
                    ColorsImpl,
                    BUCKETS_COUNT,
                >(args);
            } else {
                run_querier_from_args::<
                    CanonicalNtHashIteratorFactory,
                    cn_rkhash::u64::CanonicalRabinKarpHashFactory,
                    ColorsImpl,
                    BUCKETS_COUNT,
                >(args)
            }
        }
        HashType::RabinKarp128 => {
            if args.common_args.forward_only {
                run_querier_from_args::<
                    ForwardNtHashIteratorFactory,
                    fw_rkhash::u128::ForwardRabinKarpHashFactory,
                    ColorsImpl,
                    BUCKETS_COUNT,
                >(args);
            } else {
                run_querier_from_args::<
                    CanonicalNtHashIteratorFactory,
                    cn_rkhash::u128::CanonicalRabinKarpHashFactory,
                    ColorsImpl,
                    BUCKETS_COUNT,
                >(args)
            }
        }
        HashType::Auto => {
            unreachable!()
        }
    }
}