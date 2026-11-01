# rt

Práctica de decisión y reflejos para la terminal, pensada para ratos de espera
(mientras compila un proyecto o corren pruebas) en un panel de Windows Terminal.
Sesiones de 1 a 2 minutos, sin sonido y sin consumo de CPU en reposo.

## Compilar

Requiere Rust estable. Con el toolchain GNU (`stable-x86_64-pc-windows-gnu`) o MSVC:

```bash
cargo build --release
```

El ejecutable queda en `target\release\rt.exe`. Para tenerlo en el `PATH`
(lo copia a `%USERPROFILE%\.cargo\bin`):

```bash
cargo install --path .
```

> `Cargo.toml` fija `parking_lot 0.12.3` / `parking_lot_core 0.9.10`. Las versiones
> nuevas usan `raw-dylib` y en el toolchain GNU necesitan `dlltool` con ensamblador,
> que rustup no instala.

Pruebas y lint:

```bash
cargo test
```

```bash
cargo clippy --all-targets
```

## Uso

```
rt                     menú
rt --quick             empieza una sesión de inmediato
rt --baseline          prueba base de Decisión: 30 intentos, sin adaptación
rt --mode blancos      modo Blancos (los modos también se eligen en el menú con ←→)
rt --mode flick        un blanco a la vez, en las 8 direcciones
rt --mode seguimiento  mantener el cursor sobre un blanco que se mueve
rt --mode reaccion     reacción pura
rt --mode gonogo       responder solo al relleno
rt export              intentos a CSV por stdout
rt export -o out.csv   intentos a un archivo
rt --theme cyber       camuflaje de terminal en cian y magenta
rt --theme dark        estilo de recuadros con fondo degradado
rt --theme mono        un solo color
rt --plain             sin fondo ni efectos: estilo sobrio
rt --trials 10 --fixed --timeout 1500
rt --pixel             intenta mouse en píxeles (ver abajo)
```

En cada intento aparece un punto de fijación y, tras 0.5–2 s, de 3 a 5 recuadros.
El objetivo es el bloque **relleno** (verde); los distractores son **huecos** (rojos).
La diferencia no depende solo del color: con `--theme mono` se distinguen por el relleno.
Responde con un clic sobre el objetivo o con la tecla de su número.
En el ~17% de los intentos no hay objetivo: lo correcto es no responder.

### Apariencia

**Camuflaje de terminal (por defecto, temas `matrix` y `cyber`).** Todo parece salida
de comandos, para pasar desapercibido junto a terminales que de verdad están trabajando:

- de fondo, salida falsa y tenue de `cargo`, pruebas, logs, `git log`, volcados hex y
  código; avanza una línea cada 1–3 s, también mientras juegas (`"ambient_motion": false`
  la deja quieta durante el juego);
- el prompt imita al tuyo (`PS C:
uta>`; en paneles estrechos, solo la última carpeta);
- el menú parece un `rt --help`, la cuenta regresiva un `cargo run`, la barra de estado
  el progreso de `cargo` (`Building [=====>    ] 12/20`), la pausa un proceso detenido
  (`[1]+  Stopped`), el resumen la salida de `cargo test` (`test result: ok. …`) y el
  progreso un `git log` con un hash por sesión;
- el objetivo es un bloque de código "seleccionado" (fondo de color) y los distractores
  un marco de líneas finas (`┌──┐`); se distinguen por la forma, no solo por el color;
- el punto de fijación es un cursor `_`, y los efectos usan `1`/`0`, `#` y `x`.

`matrix` usa verde fósforo (objetivo verde, distractor ámbar) y `cyber` cian y magenta.
Ambos dejan el fondo de tu terminal tal cual. `"background": "plain"` o `"none"` quita
el texto de fondo.

**Estilo de recuadros (`--theme dark`).** El fondo es un degradado oscuro con una rejilla de puntos muy tenue, los
menús van en paneles con marco redondeado y la cuenta regresiva, el puntaje y el logo
usan cifras grandes de medio bloque (solo si caben enteras).

Cada respuesta deja un efecto de unos 380 ms:

| efecto | cuándo |
|---|---|
| anillo verde y destello de puntos | acierto |
| sombreado rojo (`▒░`) sobre el recuadro elegido | error, falsa alarma o anticipación |
| contorno tenue donde estaba el objetivo | omisión o error |
| aspa roja donde se hizo clic | clic en el vacío (modo Blancos) |

En los dos estilos, los efectos solo piden un cuadro cada 60 ms mientras duran y no tocan la medición: el
tiempo de cada estímulo se marca al dibujarlo, antes de cualquier efecto. Con `--plain`,
o con `"background": "none"` y `"effects": false`, vuelve el estilo sobrio original.
`"background": "plain"` deja el degradado sin la rejilla.

### Modos

| modo | qué entrena | cómo termina | métrica principal |
|---|---|---|---|
| decisión | elegir el objetivo entre distractores, y no responder sin objetivo | 20 intentos | puntaje (eficiencia inversa) |
| blancos | velocidad de clic con varios blancos; la dificultad aleja los blancos del centro | 30 s | puntaje |
| flick | un movimiento largo y preciso a la vez | 30 blancos | puntaje |
| seguimiento | mantener el cursor sobre algo que se mueve | 30 s | % del tiempo encima |
| reacción | velocidad pura, sin decisión ni puntería | 20 intentos | puntaje |
| go/no-go | control de impulsos: responder al relleno, no al hueco | 30 intentos | comisiones y puntaje |

- **Flick**: un solo blanco lejos del cursor; las 8 direcciones salen repartidas por
  igual. Un clic fuera es un fallo y el blanco se queda. Si no lo tocas en 3 s, es una
  omisión y aparece otro. Es el modo que mejor alimenta el análisis por dirección. Tiene
  dificultad propia (ver abajo).
- **Seguimiento**: el recuadro avanza a 12 celdas/s, rebota y cambia de rumbo. Cada
  segundo se guarda la fracción del tiempo con el cursor encima y la distancia media.
  Es el único modo que redibuja seguido (unas 30 veces por segundo) mientras juegas.
- **Reacción pura** y **Go/No-Go**: un recuadro en el centro tras 0.8–2.5 s. Se responde
  con espacio, un número o un clic. En Go/No-Go, el 25% de los intentos son no-go
  (hueco): responder ahí es una comisión. Plazo para responder: 1 s.

### Puntería por dirección

`a` en el resumen o "puntería" en el menú analiza las últimas 30 sesiones con mouse
(decisión, blancos y flick). Para cada movimiento usa dónde estaba el cursor al aparecer
el blanco, el centro del blanco y el punto del clic, y muestra por cada una de las 8
direcciones (→ ↗ ↑ ↖ ← ↙ ↓ ↘):

- el porcentaje de acierto y la mediana de tiempo, en una rosa de direcciones
  (verde ≥85%, rojo <70%);
- **pasarse**: el error a lo largo del movimiento; positivo es que te pasas del blanco,
  negativo que te quedas corto;
- **lateral**: el error de lado; positivo es a la derecha de la trayectoria;
- conclusiones en palabras: hacia dónde fallas más, hacia dónde eres más lento y si te
  pasas, te quedas corto o te desvías.

Las distancias van en anchos de celda, con la vertical contada doble (una celda es el
doble de alta que de ancha). Con mouse en celdas la resolución es de una celda, así que
los sesgos de menos de 0.3 se ignoran.

### Dificultad de Blancos

En el menú, con el modo en blancos, `dificultad` (←→) recorre **fácil, normal, difícil,
experto y adaptativa**. Cada dificultad es un nivel de 0 a 9 que decide qué tan lejos
del centro suelen aparecer los blancos: la distancia se sortea como `u^e`, con `u` al
azar entre 0 y 1 y un exponente que baja con el nivel (2.5 en el 0, 0.35 en el 9). En
fácil casi todos salen cerca del centro; en experto, la mayoría cerca del borde.

| dificultad | nivel | distancia media al centro |
|---|---|---|
| fácil | 1 | ~30% del radio |
| normal | 3 | ~40% |
| difícil | 6 | ~60% |
| experto | 9 | ~75% |

En **adaptativa** el nivel lo pone tu rendimiento, con la misma escalera que Decisión:
3 aciertos rápidos seguidos (hasta `targets_adapt_ms`, 900 ms por defecto) suben un
nivel; un clic en el vacío o un acierto muy lento (más de 1.5 veces ese tiempo) baja
uno; un acierto algo lento solo corta la racha. El blanco tocado se recoloca con el
nivel ya actualizado. La barra de estado muestra el nivel actual.

El resumen de Blancos compara la mediana de los aciertos **cerca del centro** (radio
< 0.5) con los de **lejos**, y cada intento guarda su radio (0 centro, 1 borde) en el
historial y en el CSV (`radius`). Una sesión solo se compara con otra de la misma
dificultad (y en fija, del mismo nivel), porque la distancia cambia el tiempo.

### Dificultad de Flick

Igual que en Blancos, `dificultad` (←→) recorre fácil, normal, difícil, experto y
adaptativa, con su propia configuración. Aquí el nivel hace dos cosas: **achica el
blanco** y lo hace **salir más lejos del cursor**.

| nivel | dificultad | blanco | distancia (parte del rango) |
|---|---|---|---|
| 1 | fácil | 6x3 | 5–45% |
| 3 | normal | 5x2 | 15–65% |
| 6 | difícil | 3x1 | 40–90% |
| 9 | experto | 2x1 | 60–100% |

El rango va de 8 celdas a un 60% de la diagonal del panel. Si en una dirección no cabe la
distancia del nivel, el blanco se acerca lo justo. En adaptativa, la escalera es la de
Blancos: 3 aciertos rápidos seguidos (hasta `flick_adapt_ms`, 850 ms) suben un nivel; un
fallo, dejar pasar el blanco o un acierto muy lento bajan uno. Como cambian tamaño y
distancia, el **rendimiento** (bits/s, ley de Fitts) es la mejor métrica para comparar
entre niveles.

### Mejoras de medición

- **Calentamiento**: los primeros 3 intentos de cada sesión (`warmup_trials`, o
  `--warmup N`; 0 lo quita) se juegan y se guardan marcados, pero no entran a las
  estadísticas, al análisis de puntería ni a la cuenta para terminar: una sesión de 20
  sigue teniendo 20 intentos que cuentan. En Blancos y Seguimiento el reloj arranca al
  acabar el calentamiento. La escalera adaptativa sí se mueve durante el calentamiento,
  así que llegas a la parte que cuenta ya en tu nivel. En Decisión y Go/No-Go el
  calentamiento siempre tiene objetivo, para no alterar la proporción de intentos sin
  objetivo. La barra muestra `calentamiento 1/3` (o `Warming` en el estilo de código).
- **La escalera adaptativa retoma donde quedaste**: en Decisión y en Blancos, cada
  sesión adaptativa empieza un nivel por debajo del nivel final de la anterior, en vez
  de volver a 0.
- **Rendimiento (ley de Fitts)** en Blancos y Flick: para cada acierto,
  `log2(distancia / ancho + 1)` bits divididos entre el tiempo, en bits por segundo.
  Junta distancia, tamaño y velocidad en un solo número, así que sirve para comparar
  sesiones aunque los blancos salgan a distancias distintas. Mayor es mejor.

### Modo Blancos

Hay 3 recuadros verdes a la vez durante 30 s. Al hacer clic en uno, solo ese cambia de
lugar, y nunca aparece bajo el cursor. Es solo con mouse. Un clic en el vacío cuenta
como fallo. Cada clic es un intento: su tiempo se mide desde que se dibujó el cuadro
anterior, y la distancia se mide desde el cursor en ese momento hasta el blanco, así que
sirve para ver el tiempo frente a la distancia y el tamaño. El reloj se detiene en pausa.
El resumen muestra aciertos, fallos, precisión, ritmo (aciertos por segundo), mediana
entre aciertos y el mismo puntaje combinado. Se configura con `targets_count`,
`targets_secs`, `targets_w` y `targets_h`.

Teclas: `1`–`9` responder · `Esc` pausa (otra vez `Esc` termina la sesión) · `q` sale
de inmediato · en el resumen `Enter` repite y `p` abre el progreso · en el progreso
`←→` o `Tab` cambia entre decisión, baseline y blancos. En el menú, `←→` cambia el modo
y la dificultad.

El resumen compara cada métrica con la sesión anterior del mismo modo y dificultad
(verde si mejora, rojo si empeora). El progreso tiene una vista por modo (y una para las baselines de decisión). Muestra, por métrica, el último valor,
la tendencia frente a la mediana de las 5 sesiones anteriores, el rango y una gráfica
de barras que crece en altura cuando el panel es grande.

Si sales a mitad de una sesión, se guarda marcada como incompleta y no entra al progreso.
Si la terminal pierde el foco o cambia de tamaño durante un intento, ese intento se
descarta y no cuenta; al perder el foco la sesión se pausa.

### Resultados

| resultado | cuándo |
|---|---|
| acierto | eligió el objetivo |
| error | eligió un distractor |
| omisión | se acabó el tiempo con objetivo presente |
| falsa alarma | respondió sin objetivo presente |
| rechazo correcto | no respondió sin objetivo presente |
| anticipación | respondió antes del estímulo o en menos de 120 ms |
| fallo | clic en el vacío (modo Blancos) |

Las anticipaciones no cuentan como acierto ni entran a los tiempos; se reportan aparte.
El resumen muestra mediana, media, mejor tiempo, precisión (aciertos / intentos con
objetivo), conteos y el **puntaje** combinado: eficiencia inversa = mediana de aciertos /
precisión, en ms. Menor es mejor, y así ir rápido con muchos errores no parece mejora.

Los tiempos se toman con `Instant`: inicio justo después de dibujar y vaciar el cuadro
del estímulo, fin al llegar el evento. Incluyen la latencia de la terminal, así que
sirven para compararte contigo mismo, no con otras personas ni con otros equipos.

### Dificultad

- **fija**: usa `box_w`, `box_h`, `min_boxes`–`max_boxes` y `timeout_ms` de la configuración.
- **adaptativa** (por defecto): escalera de 10 niveles. Sube tras 3 aciertos seguidos y
  baja tras un fallo (error, omisión, falsa alarma o anticipación), lo que la mantiene
  cerca del 80% de aciertos. Los rechazos correctos no la mueven. Cada nivel achica los
  recuadros, suma opciones y recorta el tiempo límite (de 2000 a 1100 ms).

Se cambia en el menú, con `--fixed` / `--adaptive` o en el archivo de configuración.

## Datos

Todo vive en `%APPDATA%\term-aim\` (`~/.config/term-aim` fuera de Windows):

- `history.jsonl`: una línea por sesión con fecha (UTC), modo, dificultad, configuración,
  si es baseline o incompleta, intentos descartados, nivel final, estadísticas e intentos.
- `config.json` (opcional): cualquier subconjunto de estas claves.

```json
{
  "mode": "decision",
  "trials": 20,
  "timeout_ms": 2000,
  "anticipation_ms": 120,
  "catch_ratio": 0.17,
  "fixation_min_ms": 500,
  "fixation_max_ms": 2000,
  "min_boxes": 3,
  "max_boxes": 5,
  "box_w": 6,
  "box_h": 3,
  "difficulty": "adaptive",
  "start_level": 0,
  "theme": "matrix",
  "background": "grid",
  "effects": true,
  "target_fill": "filled",
  "distractor_fill": "hollow",
  "target_color": "green",
  "distractor_color": "red",
  "pixel_mouse": false,
  "targets_count": 3,
  "targets_secs": 30,
  "targets_w": 6,
  "targets_h": 3,
  "targets_difficulty": "adaptive",
  "targets_level": 3,
  "targets_adapt_ms": 900,
  "flick_trials": 30,
  "flick_timeout_ms": 3000,
  "flick_difficulty": "adaptive",
  "flick_level": 3,
  "flick_adapt_ms": 850,
  "tracking_secs": 30,
  "tracking_w": 6,
  "tracking_h": 3,
  "tracking_speed": 12.0,
  "reaction_trials": 20,
  "gonogo_trials": 30,
  "nogo_ratio": 0.25,
  "go_timeout_ms": 1000,
  "ambient_motion": true,
  "warmup_trials": 3
}
```

`target_color` y `distractor_color` solo se usan con `--theme dark`.
Los colores aceptan `green`, `red`, `blue`, `yellow`, `cyan`, `magenta`, `gray`,
`white` (en tonos apagados) o `#rrggbb`. La baseline ignora la configuración salvo
la apariencia, para que sea comparable entre semanas.

Columnas del CSV: `session_date, mode, baseline, incomplete, difficulty, trial, level,
n_boxes, target_present, target_w, target_h, cursor_dist, rt_ms, outcome, method,
anticipation, start_x, start_y, target_x, target_y, click_x, click_y, direction,
overshoot, lateral, score, radius, warmup`. `start_*` es el cursor al aparecer el blanco, `target_*` su
centro (en un clic fallido, el del blanco más cercano), `click_*` el clic, `direction`
una de las 8 direcciones, `overshoot`/`lateral` el error de puntería descrito arriba y
`score` la fracción de tiempo encima en Seguimiento y `radius` la distancia del blanco al
centro en Blancos (0 a 1). `warmup` marca los intentos de calentamiento (fíltralos
para analizar). `cursor_dist` es la distancia del cursor al centro del objetivo cuando
aparece el estímulo, en anchos de celda (la vertical cuenta doble por la forma de la
celda); queda vacía si el mouse no se movió dentro de la ventana o si no había objetivo.

## Mouse en píxeles

Con `--pixel` (o `"pixel_mouse": true`) se pide el modo SGR-Pixels (1016). Si llegan
coordenadas mayores que la cuadrícula y la terminal informa su tamaño en píxeles,
la posición del cursor se usa con precisión de subcelda; si no, todo sigue en celdas.
En Windows crossterm lee el mouse con la API de consola, que siempre entrega celdas,
así que ahí esta opción no tiene efecto. Nada depende de ella.

## Estructura

```
src/
  main.rs          argumentos y comandos
  config.rs        configuración y CLI
  term.rs          pantalla alterna, modo raw, restauración (también en panic)
  input.rs         eventos: solo Press, movimiento del mouse, foco, hora de llegada
  render.rs        pantallas del estilo de recuadros y primitivas compartidas
  render_code.rs   pantallas del estilo de código (camuflaje de terminal)
  ambient.rs       salida falsa de fondo
  theme.rs         paletas
  app.rs           pantallas, sesión y bucle dirigido por eventos
  modes/mod.rs     trait GameMode y tipos comunes de intento
  modes/layout.rs  colocación de recuadros sin solaparse
  modes/decision.rs  modo Decisión
  modes/targets.rs   modo Blancos
  modes/flick.rs     modo Flick
  modes/tracking.rs  modo Seguimiento
  modes/single.rs    Reacción pura y Go/No-Go
  aim.rs           análisis de puntería por dirección
  report.rs        qué métricas muestra cada modo
  difficulty.rs    niveles y escalera
  stats.rs         estadísticas
  history.rs       JSON Lines y CSV
```

Para añadir un modo, implementa `GameMode` en
`src/modes/`, añádelo a `GameKind` y créalo en `modes::create`. La sesión, la pausa,
el descarte, el reloj, las estadísticas y el historial no cambian.
