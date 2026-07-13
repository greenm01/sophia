#define _POSIX_C_SOURCE 200809L

#include <errno.h>
#include <fcntl.h>
#include <gbm.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <drm_fourcc.h>
#include <wayland-client.h>

#include "linux-dmabuf-v1-client-protocol.h"
#include "xdg-shell-client-protocol.h"

enum {
    BUFFER_COUNT = 2,
    FRAME_LIMIT = 1000,
    MAX_DIMENSION = 8192,
};

struct producer;

struct dmabuf_buffer {
    struct gbm_bo *bo;
    struct wl_buffer *buffer;
    bool released;
};

struct producer {
    struct wl_display *display;
    struct wl_registry *registry;
    struct wl_compositor *compositor;
    struct xdg_wm_base *wm_base;
    struct zwp_linux_dmabuf_v1 *dmabuf;
    struct wl_surface *surface;
    struct xdg_surface *xdg_surface;
    struct xdg_toplevel *toplevel;
    struct gbm_device *gbm;
    int render_fd;
    uint32_t width;
    uint32_t height;
    bool configured;
    bool closed;
    bool failed;
    struct dmabuf_buffer buffers[BUFFER_COUNT];
};

struct frame_wait {
    bool done;
};

static void fail(struct producer *producer, const char *message) {
    if (!producer->failed) {
        fprintf(stderr, "sophia DMA-BUF producer: %s\n", message);
    }
    producer->failed = true;
}

static void wm_base_ping(
    void *data,
    struct xdg_wm_base *wm_base,
    uint32_t serial
) {
    (void)data;
    xdg_wm_base_pong(wm_base, serial);
}

static const struct xdg_wm_base_listener WM_BASE_LISTENER = {
    .ping = wm_base_ping,
};

static void xdg_surface_configure(
    void *data,
    struct xdg_surface *xdg_surface,
    uint32_t serial
) {
    struct producer *producer = data;
    xdg_surface_ack_configure(xdg_surface, serial);
    producer->configured = true;
}

static const struct xdg_surface_listener XDG_SURFACE_LISTENER = {
    .configure = xdg_surface_configure,
};

static void xdg_toplevel_configure(
    void *data,
    struct xdg_toplevel *toplevel,
    int32_t width,
    int32_t height,
    struct wl_array *states
) {
    struct producer *producer = data;
    (void)toplevel;
    (void)states;
    if (width <= 0 || height <= 0 || width > MAX_DIMENSION || height > MAX_DIMENSION) {
        fail(producer, "compositor configured an unsupported DMA-BUF size");
        return;
    }
    producer->width = (uint32_t)width;
    producer->height = (uint32_t)height;
}

static void xdg_toplevel_close(void *data, struct xdg_toplevel *toplevel) {
    struct producer *producer = data;
    (void)toplevel;
    producer->closed = true;
}

static const struct xdg_toplevel_listener XDG_TOPLEVEL_LISTENER = {
    .configure = xdg_toplevel_configure,
    .close = xdg_toplevel_close,
};

static void buffer_release(void *data, struct wl_buffer *buffer) {
    struct dmabuf_buffer *dmabuf_buffer = data;
    (void)buffer;
    dmabuf_buffer->released = true;
}

static const struct wl_buffer_listener BUFFER_LISTENER = {
    .release = buffer_release,
};

static void frame_done(void *data, struct wl_callback *callback, uint32_t time) {
    struct frame_wait *wait = data;
    (void)time;
    wait->done = true;
    wl_callback_destroy(callback);
}

static const struct wl_callback_listener FRAME_LISTENER = {
    .done = frame_done,
};

static void registry_global(
    void *data,
    struct wl_registry *registry,
    uint32_t name,
    const char *interface,
    uint32_t version
) {
    struct producer *producer = data;
    if (strcmp(interface, wl_compositor_interface.name) == 0) {
        if (version < 4) {
            fail(producer, "wl_compositor lacks damage_buffer support");
            return;
        }
        producer->compositor = wl_registry_bind(
            registry,
            name,
            &wl_compositor_interface,
            4
        );
    } else if (strcmp(interface, xdg_wm_base_interface.name) == 0) {
        producer->wm_base = wl_registry_bind(
            registry,
            name,
            &xdg_wm_base_interface,
            1
        );
    } else if (strcmp(interface, zwp_linux_dmabuf_v1_interface.name) == 0) {
        if (version < 3) {
            fail(producer, "linux-dmabuf lacks create_immed support");
            return;
        }
        producer->dmabuf = wl_registry_bind(
            registry,
            name,
            &zwp_linux_dmabuf_v1_interface,
            3
        );
    }
}

static void registry_global_remove(
    void *data,
    struct wl_registry *registry,
    uint32_t name
) {
    (void)data;
    (void)registry;
    (void)name;
}

static const struct wl_registry_listener REGISTRY_LISTENER = {
    .global = registry_global,
    .global_remove = registry_global_remove,
};

static int dispatch_until(
    struct producer *producer,
    const bool *condition
) {
    while (!*condition && !producer->closed && !producer->failed) {
        if (wl_display_dispatch(producer->display) < 0) {
            fail(producer, "Wayland display disconnected");
            return -1;
        }
    }
    return producer->failed || producer->closed ? -1 : 0;
}

static int paint_buffer(
    struct producer *producer,
    struct dmabuf_buffer *buffer,
    unsigned int frame
) {
    uint32_t stride = 0;
    void *map_data = NULL;
    uint32_t *pixels = gbm_bo_map(
        buffer->bo,
        0,
        0,
        producer->width,
        producer->height,
        GBM_BO_TRANSFER_WRITE,
        &stride,
        &map_data
    );
    if (pixels == NULL) {
        fail(producer, "could not map linear GBM buffer");
        return -1;
    }
    for (uint32_t y = 0; y < producer->height; y++) {
        for (uint32_t x = 0; x < producer->width; x++) {
            const uint32_t red = (uint32_t)((frame * 29U + x) & 0xffU);
            const uint32_t green = (uint32_t)((frame * 17U + y) & 0xffU);
            pixels[(y * stride / 4U) + x] = 0xff000000U | (red << 16U) | (green << 8U) | 0x55U;
        }
    }
    gbm_bo_unmap(buffer->bo, map_data);
    return 0;
}

static int create_buffer(
    struct producer *producer,
    struct dmabuf_buffer *buffer,
    unsigned int frame
) {
    const uint64_t linear_modifier = DRM_FORMAT_MOD_LINEAR;
    buffer->bo = gbm_bo_create_with_modifiers2(
        producer->gbm,
        producer->width,
        producer->height,
        DRM_FORMAT_XRGB8888,
        &linear_modifier,
        1,
        0
    );
    if (buffer->bo == NULL) {
        char message[160];
        snprintf(
            message,
            sizeof(message),
            "could not allocate linear XRGB GBM buffer: %s",
            strerror(errno)
        );
        fail(producer, message);
        return -1;
    }
    if (gbm_bo_get_modifier(buffer->bo) != DRM_FORMAT_MOD_LINEAR) {
        fail(producer, "GBM did not allocate the requested linear DMA-BUF");
        return -1;
    }
    if (paint_buffer(producer, buffer, frame) != 0) {
        return -1;
    }

    const int dma_fd = gbm_bo_get_fd(buffer->bo);
    if (dma_fd < 0) {
        fail(producer, "could not export GBM buffer as DMA-BUF");
        return -1;
    }
    const uint32_t stride = gbm_bo_get_stride(buffer->bo);
    struct zwp_linux_buffer_params_v1 *params =
        zwp_linux_dmabuf_v1_create_params(producer->dmabuf);
    zwp_linux_buffer_params_v1_add(
        params,
        dma_fd,
        0,
        0,
        stride,
        (uint32_t)(DRM_FORMAT_MOD_LINEAR >> 32U),
        (uint32_t)DRM_FORMAT_MOD_LINEAR
    );
    close(dma_fd);
    buffer->buffer = zwp_linux_buffer_params_v1_create_immed(
        params,
        producer->width,
        producer->height,
        DRM_FORMAT_XRGB8888,
        0
    );
    zwp_linux_buffer_params_v1_destroy(params);
    if (buffer->buffer == NULL) {
        fail(producer, "could not create Wayland DMA-BUF buffer");
        return -1;
    }
    wl_buffer_add_listener(buffer->buffer, &BUFFER_LISTENER, buffer);
    buffer->released = true;
    return 0;
}

static int initialize_wayland(struct producer *producer) {
    producer->registry = wl_display_get_registry(producer->display);
    wl_registry_add_listener(producer->registry, &REGISTRY_LISTENER, producer);
    if (wl_display_roundtrip(producer->display) < 0 || producer->failed) {
        return -1;
    }
    if (producer->compositor == NULL || producer->wm_base == NULL || producer->dmabuf == NULL) {
        fail(producer, "compositor, xdg-shell, or linux-dmabuf global is unavailable");
        return -1;
    }
    xdg_wm_base_add_listener(producer->wm_base, &WM_BASE_LISTENER, producer);
    producer->surface = wl_compositor_create_surface(producer->compositor);
    producer->xdg_surface = xdg_wm_base_get_xdg_surface(producer->wm_base, producer->surface);
    xdg_surface_add_listener(producer->xdg_surface, &XDG_SURFACE_LISTENER, producer);
    producer->toplevel = xdg_surface_get_toplevel(producer->xdg_surface);
    xdg_toplevel_add_listener(producer->toplevel, &XDG_TOPLEVEL_LISTENER, producer);
    xdg_toplevel_set_title(producer->toplevel, "Sophia DMA-BUF producer");
    xdg_toplevel_set_app_id(producer->toplevel, "org.sophia.dmabuf-producer");
    wl_surface_commit(producer->surface);
    if (dispatch_until(producer, &producer->configured) != 0) {
        return -1;
    }
    if (producer->width == 0 || producer->height == 0) {
        fail(producer, "compositor did not provide a DMA-BUF size");
        return -1;
    }
    return 0;
}

static int run_frames(struct producer *producer, unsigned int frame_count) {
    for (unsigned int frame = 0; frame < frame_count; frame++) {
        struct dmabuf_buffer *buffer = &producer->buffers[frame % BUFFER_COUNT];
        if (!buffer->released && dispatch_until(producer, &buffer->released) != 0) {
            return -1;
        }
        if (paint_buffer(producer, buffer, frame + BUFFER_COUNT) != 0) {
            return -1;
        }
        buffer->released = false;
        struct frame_wait wait = {0};
        struct wl_callback *callback = wl_surface_frame(producer->surface);
        wl_callback_add_listener(callback, &FRAME_LISTENER, &wait);
        wl_surface_attach(producer->surface, buffer->buffer, 0, 0);
        wl_surface_damage_buffer(
            producer->surface,
            0,
            0,
            (int32_t)producer->width,
            (int32_t)producer->height
        );
        wl_surface_commit(producer->surface);
        if (wl_display_flush(producer->display) < 0 && errno != EAGAIN) {
            fail(producer, "could not flush Wayland DMA-BUF frame");
            return -1;
        }
        if (dispatch_until(producer, &wait.done) != 0) {
            return -1;
        }
    }
    return 0;
}

static void cleanup(struct producer *producer) {
    for (size_t index = 0; index < BUFFER_COUNT; index++) {
        if (producer->buffers[index].buffer != NULL) {
            wl_buffer_destroy(producer->buffers[index].buffer);
        }
        if (producer->buffers[index].bo != NULL) {
            gbm_bo_destroy(producer->buffers[index].bo);
        }
    }
    if (producer->toplevel != NULL) {
        xdg_toplevel_destroy(producer->toplevel);
    }
    if (producer->xdg_surface != NULL) {
        xdg_surface_destroy(producer->xdg_surface);
    }
    if (producer->surface != NULL) {
        wl_surface_destroy(producer->surface);
    }
    if (producer->dmabuf != NULL) {
        zwp_linux_dmabuf_v1_destroy(producer->dmabuf);
    }
    if (producer->wm_base != NULL) {
        xdg_wm_base_destroy(producer->wm_base);
    }
    if (producer->compositor != NULL) {
        wl_compositor_destroy(producer->compositor);
    }
    if (producer->registry != NULL) {
        wl_registry_destroy(producer->registry);
    }
    if (producer->display != NULL) {
        wl_display_disconnect(producer->display);
    }
    if (producer->gbm != NULL) {
        gbm_device_destroy(producer->gbm);
    }
    if (producer->render_fd >= 0) {
        close(producer->render_fd);
    }
}

static int parse_frames(const char *value, unsigned int *frames) {
    char *end = NULL;
    errno = 0;
    const unsigned long parsed = strtoul(value, &end, 10);
    if (errno != 0 || end == value || *end != '\0' || parsed < 2 || parsed > FRAME_LIMIT) {
        return -1;
    }
    *frames = (unsigned int)parsed;
    return 0;
}

int main(int argc, char **argv) {
    const char *render_node = NULL;
    unsigned int frames = 3;
    for (int index = 1; index < argc; index++) {
        if (strcmp(argv[index], "--render-node") == 0 && index + 1 < argc) {
            render_node = argv[++index];
        } else if (strcmp(argv[index], "--frames") == 0 && index + 1 < argc) {
            if (parse_frames(argv[++index], &frames) != 0) {
                fprintf(stderr, "--frames must be an integer from 2 to %d\n", FRAME_LIMIT);
                return EXIT_FAILURE;
            }
        } else {
            fprintf(stderr, "usage: %s --render-node /dev/dri/renderD* [--frames 2..%d]\n", argv[0], FRAME_LIMIT);
            return EXIT_FAILURE;
        }
    }
    if (render_node == NULL) {
        fprintf(stderr, "--render-node is required\n");
        return EXIT_FAILURE;
    }

    struct producer producer = {.render_fd = -1};
    producer.render_fd = open(render_node, O_RDWR | O_CLOEXEC);
    if (producer.render_fd < 0) {
        perror("open render node");
        return EXIT_FAILURE;
    }
    producer.gbm = gbm_create_device(producer.render_fd);
    if (producer.gbm == NULL) {
        fprintf(stderr, "could not create GBM device\n");
        cleanup(&producer);
        return EXIT_FAILURE;
    }
    producer.display = wl_display_connect(NULL);
    if (producer.display == NULL) {
        fprintf(stderr, "could not connect to WAYLAND_DISPLAY\n");
        cleanup(&producer);
        return EXIT_FAILURE;
    }
    if (initialize_wayland(&producer) == 0) {
        for (size_t index = 0; index < BUFFER_COUNT; index++) {
            if (create_buffer(&producer, &producer.buffers[index], (unsigned int)index) != 0) {
                break;
            }
        }
        if (!producer.failed && run_frames(&producer, frames) == 0) {
            printf("sophia_dmabuf_producer schema=1 status=complete frames=%u\n", frames);
        }
    }
    const int status = producer.failed || producer.closed ? EXIT_FAILURE : EXIT_SUCCESS;
    cleanup(&producer);
    return status;
}
