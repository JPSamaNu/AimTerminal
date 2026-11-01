# rt

Práctica de decisión, reflejos y puntería para la terminal, pensada para ratos de espera
(mientras compila un proyecto o corren pruebas) en un panel de Windows Terminal.
Sesiones de 1 a 2 minutos, sin sonido y sin consumo de CPU en reposo. Por defecto se
camufla: todo parece salida de comandos.

El detalle de cada modo, las métricas, la configuración y los formatos de datos está en
[docs/REFERENCIA.md](docs/REFERENCIA.md).

## Instalar

**Sin compilar:** `bin/rt.exe` es el ejecutable listo para usar, actualizado en cada
commit. Solo depende de DLLs de Windows: cópialo a una carpeta del `PATH` o córrelo con
su ruta. También se descarga de la pestaña Actions de GitHub (artefacto `rt-windows`).

**Compilando** (Rust estable, toolchain GNU o MSVC):

```bash
cargo install --path .
```

## Empezar

```bash
rt
```

```bash
rt --plan
```

`rt` abre el menú. `rt --plan` arma una rutina de 3 bloques cortos según tus puntos
débiles. Otras formas de arrancar:

```
rt --quick                 sesión inmediata del modo por defecto
rt --mode flick --quick    sesión inmediata de un modo
rt --baseline              prueba base fija, para comparar semana a semana
rt export -o intentos.csv  todos los intentos a CSV
rt data                    dónde se guardan historial y configuración
rt --help                  todas las opciones
```

## Teclas

| dónde | teclas |
|---|---|
| menú | `↑↓` elegir · `Enter` abrir · `←→` cambiar modo y dificultad · `q` salir |
| juego | clic o `1`–`9` responder · `espacio` en reacción y go/no-go · `Esc` pausa |
| pausa | `Enter` seguir · `Esc` terminar la sesión (se guarda como incompleta) |
| resumen | `Enter` otra · `p` progreso · `a` puntería por dirección · `Esc` menú |
| progreso | `←→` vista · `d` filtrar dificultad · `w` por sesión o por semana |
| cualquier pantalla | `q` sale de inmediato |

## Modos

| modo | qué entrena | sesión |
|---|---|---|
| decisión | elegir el recuadro relleno entre huecos; no responder si no hay objetivo | 20 intentos |
| blancos | clic rápido en varios blancos a la vez; cada tocado cambia de lugar | 30 s |
| flick | un movimiento largo y preciso hacia un blanco pequeño, en 8 direcciones | 30 blancos |
| seguimiento | mantener el cursor sobre un blanco que se mueve | 30 s |
| reacción | velocidad pura: responder en cuanto aparece | 20 intentos |
| go/no-go | control de impulsos: responder al relleno, no al hueco | 30 intentos |

Cada sesión empieza con 3 intentos de **calentamiento** que no cuentan.

**Dificultad.** Blancos, flick, seguimiento y go/no-go se juegan en fácil, normal,
difícil, experto o **adaptativa** (por defecto); decisión, en fija o adaptativa; reacción
no tiene. La adaptativa es una escalera de 10 niveles que sube con 3 éxitos seguidos,
baja con un fallo y retoma desde tu último nivel. En blancos el nivel aleja los blancos
del centro; en flick los achica y los aleja del cursor; en seguimiento sube velocidad y
giros; en go/no-go acorta el tiempo para responder; en decisión achica recuadros, suma
opciones y recorta el tiempo.

## Qué mide

- **Puntaje** (eficiencia inversa): mediana de los aciertos ÷ precisión, en ms. Menor es
  mejor, y así ir rápido fallando mucho no parece mejora.
- **Rendimiento** (ley de Fitts, bits/s) en blancos y flick: junta distancia, tamaño y
  tiempo, así que compara sesiones de distinta dificultad.
- **Puntería por dirección:** acierto y tiempo hacia cada una de las 8 direcciones, y si
  te pasas del blanco, te quedas corto o te desvías a un lado.
- **Progreso:** gráficas por modo y dificultad, por sesión o por semana, con la
  tendencia (mejora, estable, empeora).

Los tiempos incluyen la latencia de la terminal: sirven para compararte contigo mismo.

## Apariencia

| tema | estilo |
|---|---|
| `matrix` (por defecto) | camuflaje: menú como `--help`, progreso de `cargo`, resumen como `cargo test`, fondo de logs falsos; verde |
| `cyber` | el mismo camuflaje en cian y magenta |
| `dark` | recuadros, paneles redondeados y fondo degradado |
| `mono` | un solo color |

`rt --theme cyber` cambia el tema y `rt --plain` quita el fondo y los efectos. Cada
acierto o fallo deja un efecto breve que no afecta la medición.

## Llevar tus datos a otra computadora

El historial vive en `%APPDATA%\term-aim`, fuera del repositorio. Para que viaje con git:

```bash
rt data set C:\Proyectos\SukaTrainer\data
```

Eso fusiona tu historial en `data/` y lo recuerda. En la otra computadora, después de
clonar, corre `bin\rt.exe data set <repo>\data`. Si el repositorio es público, el
historial también lo será.

## Desarrollo

```bash
cargo test
```

```bash
cargo clippy --all-targets -- -D warnings
```

El CI de GitHub Actions corre clippy, las pruebas y la compilación en Windows y Linux en
cada push. Para añadir un modo, implementa `GameMode` en `src/modes/`, añádelo a
`GameKind` y créalo en `modes::create`. Ver [docs/REFERENCIA.md](docs/REFERENCIA.md).
