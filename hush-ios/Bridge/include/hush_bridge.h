/*
 * Hush bridge: the narrow C ABI between the Hush iOS app and the pinned
 * Bitwarden Secrets Manager SDK. See ../contract.md for the full contract.
 *
 * Caller rules:
 * - Call every function for one session from one serial queue. The same
 *   queue owns hush_session_drop. Calls on one session never overlap.
 * - Input bytes are copied before hush_session_call returns.
 * - Every HushBuffer returned by hush_session_call is released exactly once
 *   with hush_buffer_free. The bridge zeroizes the bytes before freeing.
 */
#ifndef HUSH_BRIDGE_H
#define HUSH_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque session. Owns one SDK client and one async runtime. */
typedef struct HushSession HushSession;

/* Bytes owned by the bridge. data is NULL only when len is 0. */
typedef struct HushBuffer {
  uint8_t *data;
  size_t len;
} HushBuffer;

/* Region 0 is the US cloud, 1 is the EU cloud. Any other value, or an
 * internal failure, returns NULL. */
HushSession *hush_session_new(uint32_t region);

/* Runs one UTF-8 JSON request and returns one UTF-8 JSON envelope:
 * {"ok":true,"data":...} or {"ok":false,"error":{"code":"..."}}.
 * A NULL session, NULL input, or empty input returns an invalid_request
 * envelope. Blocks the calling queue for at most about 30 seconds. */
HushBuffer hush_session_call(HushSession *session, const uint8_t *input, size_t len);

/* Destroys the session. NULL is a no-op. The pointer is invalid after. */
void hush_session_drop(HushSession *session);

/* Zeroizes and frees a buffer from hush_session_call. A buffer with NULL
 * data is a no-op. Never free the same buffer twice. */
void hush_buffer_free(HushBuffer buffer);

#ifdef __cplusplus
}
#endif

#endif /* HUSH_BRIDGE_H */
