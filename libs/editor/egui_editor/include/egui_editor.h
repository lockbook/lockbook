#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct ThemedColor ThemedColor;





/**
 * # Safety
 */
void *init_editor(void *metal_layer);

void draw_editor(void *obj);

void resize_editor(void *obj, float width, float height, float scale);

/**
 * # Safety
 */
void key_event(void *obj,
               uint16_t key_code,
               bool shift,
               bool ctrl,
               bool option,
               bool command,
               bool pressed,
               const char *characters);
