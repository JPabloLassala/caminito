/*
 * The sound server through libpulse with its GLib main loop (CMN-213): PipeWire through
 * pipewire-pulse, or PulseAudio. It follows the server's sinks, sources, sink inputs, source
 * outputs, cards and defaults into an SdStatus, and sets the Default output's volume and
 * mute, the Default input's mute and a card's profile. With no sound server it waits for
 * one, and when the server goes away it waits for it to come back.
 */

#pragma once

#include "sd-state.h"

typedef struct _SdPulse SdPulse;

/* Called on the main loop whenever the status has changed. */
typedef void (*SdPulseChanged) (SdPulse *pulse, gpointer data);

SdPulse        *sd_pulse_new             (SdPulseChanged changed,
                                          gpointer       data);
/* What the sound server has now: not running until it has answered. */
const SdStatus *sd_pulse_status          (SdPulse *pulse);
/* The Default output's loudest channel to volume, the others in proportion. Nothing without
   a Default output; the status follows once the server has. */
void            sd_pulse_set_volume      (SdPulse    *pulse,
                                          pa_volume_t volume);
void            sd_pulse_set_muted       (SdPulse *pulse,
                                          gboolean muted);
void            sd_pulse_set_input_muted (SdPulse *pulse,
                                          gboolean muted);
/* The profile of the card at an SdCard's index, by the SdProfile's name. */
void            sd_pulse_set_profile     (SdPulse    *pulse,
                                          guint32     card,
                                          const char *profile);
void            sd_pulse_free            (SdPulse *pulse);
