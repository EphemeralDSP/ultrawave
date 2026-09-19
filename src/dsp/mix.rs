/// Mix main L/R and sidechain L/R into mono. All slices must have equal length.
/// SIMD spans time, not voices: the sum keeps the scalar operation order and
/// does not fuse multiply/add or approximate the downstream sampler effects.
#[inline]
pub fn mix_inputs(inputs: [&[f32]; 4], gains: [f32; 4], output: &mut [f32]) {
    for input in inputs {
        assert_eq!(input.len(), output.len());
    }
    let mut offset = 0;

    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::*;
        // SAFETY: SSE2 is baseline on x86_64. Lengths were checked above, each
        // load/store covers four in-bounds samples, and unaligned pointers are
        // supported. Rust's slice borrows prevent output/input aliasing.
        unsafe {
            let gains = gains.map(|g| _mm_set1_ps(g));
            while offset + 4 <= output.len() {
                let mut sum = _mm_mul_ps(_mm_loadu_ps(inputs[0].as_ptr().add(offset)), gains[0]);
                for bus in 1..4 {
                    sum = _mm_add_ps(
                        sum,
                        _mm_mul_ps(_mm_loadu_ps(inputs[bus].as_ptr().add(offset)), gains[bus]),
                    );
                }
                _mm_storeu_ps(output.as_mut_ptr().add(offset), sum);
                offset += 4;
            }
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;
        // SAFETY: NEON is enabled for this target. The same checked bounds and
        // disjoint slices as the SSE2 path apply; vld1q/vst1q allow unaligned IO.
        unsafe {
            let gains = gains.map(|g| vdupq_n_f32(g));
            while offset + 4 <= output.len() {
                let mut sum = vmulq_f32(vld1q_f32(inputs[0].as_ptr().add(offset)), gains[0]);
                for bus in 1..4 {
                    sum = vaddq_f32(
                        sum,
                        vmulq_f32(vld1q_f32(inputs[bus].as_ptr().add(offset)), gains[bus]),
                    );
                }
                vst1q_f32(output.as_mut_ptr().add(offset), sum);
                offset += 4;
            }
        }
    }

    // Portable fallback and the final 0–3 frames of a SIMD block.
    for i in offset..output.len() {
        output[i] = inputs[0][i] * gains[0]
            + inputs[1][i] * gains[1]
            + inputs[2][i] * gains[2]
            + inputs[3][i] * gains[3];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_all_four_inputs_including_the_tail() {
        let mut output = [0.0; 5];
        mix_inputs(
            [&[1.0; 5], &[-0.5; 5], &[0.25; 5], &[-1.0; 5]],
            [0.5, 0.25, 0.75, 0.125],
            &mut output,
        );
        assert_eq!(output, [0.4375; 5]);
    }

    #[test]
    fn bit_exact_scalar_equivalence_with_unaligned_slices_and_tails() {
        let mut seed = 0x51a7_d123u32;
        let inputs: [Vec<f32>; 4] = std::array::from_fn(|_| {
            (0..261)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    seed as i32 as f32 / 1_000_000_000.0
                })
                .collect()
        });
        for start in 0..4 {
            for len in [0, 1, 3, 4, 5, 7, 64, 257] {
                for gains in [[0.0; 4], [1.0; 4], [0.17, 0.83, 0.37, 0.63]] {
                    let slices = inputs.each_ref().map(|s| &s[start..start + len]);
                    let mut output = vec![123.0; len + 2];
                    mix_inputs(slices, gains, &mut output[1..len + 1]);
                    for i in 0..len {
                        // Reference is the original per-frame mix, not another
                        // call into the implementation under test.
                        let scalar = slices[0][i] * gains[0]
                            + slices[1][i] * gains[1]
                            + slices[2][i] * gains[2]
                            + slices[3][i] * gains[3];
                        assert_eq!(output[i + 1].to_bits(), scalar.to_bits());
                    }
                    assert_eq!(output[0], 123.0);
                    assert_eq!(output[len + 1], 123.0);
                }
            }
        }
    }
}
