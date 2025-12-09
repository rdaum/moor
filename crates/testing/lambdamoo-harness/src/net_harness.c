/*
 * net_harness.c - Test harness network implementation for LambdaMOO
 *
 * Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com>
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * This implements the LambdaMOO network.h interface for testing purposes.
 * Instead of real networking, it:
 * - Captures output to a ring buffer
 * - Allows direct command injection
 * - Provides a non-blocking task pump
 */

#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#include "config.h"
#include "options.h"
#include "structures.h"
#include "network.h"
#include "server.h"
#include "storage.h"
#include "list.h"
#include "utils.h"
#include "program.h"

/* Output buffer configuration */
#define HARNESS_OUTPUT_BUFFER_SIZE (1024 * 1024)  /* 1MB output buffer */
#define HARNESS_MAX_CONNECTIONS 16

/* Connection state */
typedef struct harness_connection {
    int active;
    int binary;
    int input_suspended;
    server_handle shandle;
    char name[64];
} harness_connection;

/* Listener state */
typedef struct harness_listener {
    int active;
    server_listener slistener;
    char name[64];
} harness_listener;

/* Global harness state */
static struct {
    int initialized;

    /* Output capture */
    char *output_buffer;
    size_t output_len;
    size_t output_capacity;

    /* Connections */
    harness_connection connections[HARNESS_MAX_CONNECTIONS];
    int num_connections;

    /* Listeners */
    harness_listener listeners[HARNESS_MAX_CONNECTIONS];
    int num_listeners;

    /* Pending input lines (simple queue) */
    struct {
        char *line;
        int connection_id;
    } pending_input[256];
    int pending_input_head;
    int pending_input_tail;

} harness = {0};

/* ============================================================
 * Harness-specific API (called from Rust)
 * ============================================================ */

/* Initialize the harness (call before network_initialize) */
void harness_init(void) {
    memset(&harness, 0, sizeof(harness));
    harness.output_buffer = malloc(HARNESS_OUTPUT_BUFFER_SIZE);
    harness.output_capacity = HARNESS_OUTPUT_BUFFER_SIZE;
    harness.output_len = 0;
    harness.initialized = 1;
}

/* Cleanup the harness */
void harness_cleanup(void) {
    if (harness.output_buffer) {
        free(harness.output_buffer);
        harness.output_buffer = NULL;
    }
    harness.initialized = 0;
}

/* Get captured output and clear the buffer */
const char *harness_get_output(size_t *len) {
    if (len) *len = harness.output_len;
    return harness.output_buffer;
}

/* Clear the output buffer */
void harness_clear_output(void) {
    harness.output_len = 0;
    if (harness.output_buffer) {
        harness.output_buffer[0] = '\0';
    }
}

/* Create a fake connection for a player */
int harness_create_connection(Objid player) {
    for (int i = 0; i < HARNESS_MAX_CONNECTIONS; i++) {
        if (!harness.connections[i].active) {
            harness_connection *c = &harness.connections[i];
            c->active = 1;
            c->binary = 0;
            c->input_suspended = 0;
            snprintf(c->name, sizeof(c->name), "harness connection %d", i);

            /* Create the server-side connection */
            network_handle nh;
            nh.ptr = c;

            /* Use first listener if available, otherwise null */
            server_listener sl = {0};
            for (int j = 0; j < HARNESS_MAX_CONNECTIONS; j++) {
                if (harness.listeners[j].active) {
                    sl = harness.listeners[j].slistener;
                    break;
                }
            }

            c->shandle = server_new_connection(sl, nh, 0);
            harness.num_connections++;
            return i;
        }
    }
    return -1;  /* No free slots */
}

/* Queue a line of input for a connection */
int harness_queue_input(int connection_id, const char *line) {
    if (connection_id < 0 || connection_id >= HARNESS_MAX_CONNECTIONS)
        return 0;
    if (!harness.connections[connection_id].active)
        return 0;

    int next_tail = (harness.pending_input_tail + 1) % 256;
    if (next_tail == harness.pending_input_head)
        return 0;  /* Queue full */

    harness.pending_input[harness.pending_input_tail].line = strdup(line);
    harness.pending_input[harness.pending_input_tail].connection_id = connection_id;
    harness.pending_input_tail = next_tail;
    return 1;
}

/* Close a harness connection */
void harness_close_connection(int connection_id) {
    if (connection_id < 0 || connection_id >= HARNESS_MAX_CONNECTIONS)
        return;
    if (!harness.connections[connection_id].active)
        return;

    server_close(harness.connections[connection_id].shandle);
    harness.connections[connection_id].active = 0;
    harness.num_connections--;
}

/* ============================================================
 * network.h implementation
 * ============================================================ */

const char *
network_protocol_name(void)
{
    return "harness";
}

const char *
network_usage_string(void)
{
    return "";  /* No command-line arguments */
}

int
network_initialize(int argc, char **argv, Var *desc)
{
    (void)argc;
    (void)argv;

    if (!harness.initialized) {
        harness_init();
    }

    /* Return a dummy descriptor for the initial listener */
    desc->type = TYPE_INT;
    desc->v.num = 0;

    return 1;  /* Success */
}

enum error
network_make_listener(server_listener sl, Var desc,
                      network_listener *nl, Var *canon, const char **name)
{
    (void)desc;

    for (int i = 0; i < HARNESS_MAX_CONNECTIONS; i++) {
        if (!harness.listeners[i].active) {
            harness_listener *l = &harness.listeners[i];
            l->active = 1;
            l->slistener = sl;
            snprintf(l->name, sizeof(l->name), "harness listener %d", i);

            nl->ptr = l;
            *canon = new_list(0);  /* Empty canonical form */
            *name = l->name;
            harness.num_listeners++;
            return E_NONE;
        }
    }
    return E_QUOTA;  /* No free slots */
}

int
network_listen(network_listener nl)
{
    (void)nl;
    return 1;  /* Always succeeds */
}

int
network_send_line(network_handle nh, const char *line, int flush_ok)
{
    (void)nh;
    (void)flush_ok;

    size_t line_len = strlen(line);
    size_t needed = harness.output_len + line_len + 2;  /* +2 for \n and \0 */

    if (needed > harness.output_capacity) {
        /* Would overflow - in a real implementation we'd grow or flush */
        return 0;
    }

    memcpy(harness.output_buffer + harness.output_len, line, line_len);
    harness.output_len += line_len;
    harness.output_buffer[harness.output_len++] = '\n';
    harness.output_buffer[harness.output_len] = '\0';

    return 1;
}

int
network_send_bytes(network_handle nh, const char *buffer, int buflen, int flush_ok)
{
    (void)nh;
    (void)flush_ok;

    size_t needed = harness.output_len + buflen + 1;

    if (needed > harness.output_capacity) {
        return 0;
    }

    memcpy(harness.output_buffer + harness.output_len, buffer, buflen);
    harness.output_len += buflen;
    harness.output_buffer[harness.output_len] = '\0';

    return 1;
}

int
network_buffered_output_length(network_handle nh)
{
    (void)nh;
    return 0;  /* We don't buffer, we capture immediately */
}

void
network_suspend_input(network_handle nh)
{
    harness_connection *c = (harness_connection *)nh.ptr;
    if (c) c->input_suspended = 1;
}

void
network_resume_input(network_handle nh)
{
    harness_connection *c = (harness_connection *)nh.ptr;
    if (c) c->input_suspended = 0;
}

void
network_set_connection_binary(network_handle nh, int do_binary)
{
    harness_connection *c = (harness_connection *)nh.ptr;
    if (c) c->binary = do_binary;
}

int
network_process_io(int timeout)
{
    (void)timeout;

    int did_something = 0;

    /* Process any pending input */
    while (harness.pending_input_head != harness.pending_input_tail) {
        int idx = harness.pending_input_head;
        int conn_id = harness.pending_input[idx].connection_id;
        char *line = harness.pending_input[idx].line;

        harness.pending_input_head = (harness.pending_input_head + 1) % 256;

        if (conn_id >= 0 && conn_id < HARNESS_MAX_CONNECTIONS &&
            harness.connections[conn_id].active &&
            !harness.connections[conn_id].input_suspended) {

            server_receive_line(harness.connections[conn_id].shandle, line);
            did_something = 1;
        }

        free(line);
    }

    return did_something;
}

const char *
network_connection_name(network_handle nh)
{
    harness_connection *c = (harness_connection *)nh.ptr;
    return c ? c->name : "unknown";
}

Var
network_connection_options(network_handle nh, Var list)
{
    (void)nh;
    return list;  /* No special options */
}

int
network_connection_option(network_handle nh, const char *option, Var *value)
{
    (void)nh;
    (void)option;
    (void)value;
    return 0;  /* No options supported */
}

int
network_set_connection_option(network_handle nh, const char *option, Var value)
{
    (void)nh;
    (void)option;
    (void)value;
    return 0;  /* No options supported */
}

void
network_close(network_handle nh)
{
    harness_connection *c = (harness_connection *)nh.ptr;
    if (c) {
        c->active = 0;
        harness.num_connections--;
    }
}

void
network_close_listener(network_listener nl)
{
    harness_listener *l = (harness_listener *)nl.ptr;
    if (l) {
        l->active = 0;
        harness.num_listeners--;
    }
}

void
network_shutdown(void)
{
    /* Close all connections and listeners */
    for (int i = 0; i < HARNESS_MAX_CONNECTIONS; i++) {
        harness.connections[i].active = 0;
        harness.listeners[i].active = 0;
    }
    harness.num_connections = 0;
    harness.num_listeners = 0;
}

#ifdef OUTBOUND_NETWORK
enum error
network_open_connection(Var arglist, server_listener sl)
{
    (void)arglist;
    (void)sl;
    return E_PERM;  /* Not supported in harness */
}
#endif

/* ============================================================
 * Harness utility wrappers (for inline functions)
 * ============================================================ */

/* Wrapper for inline free_var() so Rust can call it */
void
harness_free_var(Var v)
{
    free_var(v);
}

/* Get the bytecode size from a compiled program.
 * Returns the size in bytes of the main bytecode vector.
 * This is the actual number of bytecode bytes, not opcodes
 * (opcodes can span multiple bytes for operands). */
unsigned
harness_get_program_bytecode_size(Program *prog)
{
    if (!prog) return 0;
    return prog->main_vector.size;
}
