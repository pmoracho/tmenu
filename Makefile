.PHONY: build-all build-es build-en

build-all: build-es build-en

build-es:
	@echo "Compilando versión en Español..."
	RUST_I18N_DEFAULT_LOCALE=es cargo build --release
	@mkdir -p dist
	@cp target/release/tmenu dist/tmenu-es
	@echo "¡Listo! Generado en dist/tmenu-es"

build-en:
	@echo "Compilando versión en Inglés..."
	RUST_I18N_DEFAULT_LOCALE=en cargo build --release
	@mkdir -p dist
	@cp target/release/tmenu dist/tmenu-en
	@echo "¡Listo! Generado en dist/tmenu-en"