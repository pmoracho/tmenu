# tmenu

tmenu es una pequeña utilidad de menú para terminal escrita en Rust.

Características principales
- Menú ligero y rápido para la terminal.
- Configuración vía archivo `tmenu.toon` (ubicado en el repositorio).

Requisitos
- Rust (toolchain estable) y `cargo` instalados.

Instalación y compilación

1. Clonar el repositorio (si aún no lo has hecho):

```bash
git clone <repositorio> && cd tmenu
```

2. Compilar en modo debug:

```bash
cargo build
```

3. Compilar en modo release (optimizado):

```bash
cargo build --release
```

Ejecución

- Ejecutar con `cargo run` (modo debug):

```bash
cargo run -- <opciones>
```

- Ejecutar el binario release generado:

```bash
./target/release/menu-rs  # o el nombre del binario según Cargo.toml
```

Archivo de configuración

- `tmenu.toon` contiene la configuración/plantilla del menú. Edita ese archivo para personalizar las entradas y el comportamiento.

Contribuciones

- ¡Contribuciones bienvenidas!
- Abre un "issue" para discutir cambios importantes y envía pull requests para mejoras o correcciones.

Licencia

- Añade la licencia que prefieras (por ejemplo, MIT o Apache-2.0). Si quieres, puedo añadir un ejemplo de archivo `LICENSE`.

Contacto

- Si necesitas ayuda adicional o quieres que adapte el README con más detalles (ejemplos de `tmenu.toon`, opciones CLI, screenshots), dímelo y lo actualizo.
