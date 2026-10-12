# ----------------------------------------------------------------------
# Downloads (test data generation and SOFA conversion)
# ----------------------------------------------------------------------
# The generator binaries used here live in their owning workspaces:
# - generate-audio-tests: math-audio/crates/math-dsp
# - generate-upmixer-golden: sotf-daw/.../sotf-plugin-upmixer
# - sofa-to-sqlite: sofa-reader
# Recipes run from the sotf root so relative output dirs
# (data_generated/, data_cached/) keep their historical locations.

[group('download')]
generate-audio-tests:
	{{cargo}} run --manifest-path ../math-audio/Cargo.toml -p math-dsp --bin generate-audio-tests --release

[group('download')]
generate-upmixer-golden:
	{{cargo}} run --manifest-path ../sotf-daw/Cargo.toml -p sotf-plugin-upmixer --bin generate-upmixer-golden --release -- --out-dir ../sotf-daw/crates/sotf-plugins/crates/sotf-plugin-upmixer/data_generated/test-plugin-upmixer

[group('download')]
download-sofa:
	mkdir -p data_cached/org.sofacoustics/mit
	wget -O data_cached/org.sofacoustics/mit/kemar_normal_pinna.sofa https://sofacoustics.org/data/database/mit/mit_kemar_normal_pinna.sofa
	wget -O data_cached/org.sofacoustics/mit/kemar_large.sofa https://sofacoustics.org/data/database/mit/mit_kemar_large_pinna.sofa

[group('download')]
convert-sofa-to-sqlite:
	@for sofa in data_cached/org.sofacoustics/mit/*.sofa; do \
		hrtfdb="$${sofa%.sofa}.hrtfdb"; \
		if [ ! -f "$$hrtfdb" ]; then \
			echo "Converting $$sofa -> $$hrtfdb"; \
			{{cargo}} run --manifest-path ../sofa-reader/Cargo.toml --bin sofa-to-sqlite --release -- "$$sofa" "$$hrtfdb"; \
		else \
			echo "Skipping $$sofa (already converted)"; \
		fi \
	done
