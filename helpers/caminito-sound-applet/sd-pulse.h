/*
 * The sound server through libpulse with its GLib main loop (CMN-213): PipeWire through
 * pipewire-pulse, or PulseAudio. It follows the server's sinks, sources, sink inputs, source
 * outputs, cards and defaults into an SdStatus, and sets the Default output's volume and
 * mute, the Default input's mute and a card's profile; for the popup (CMN-215), the
 * defaults, a device's port and the Default input's volume, and it hears the Default input
 * while the popup asks it to. With no sound server it waits for
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
/* The Default input's loudest channel to volume, the others in proportion. */
void            sd_pulse_set_input_volume (SdPulse    *pulse,
                                           pa_volume_t volume);
/* Makes the device of the side's list, by its SdDevice's name, the server's default. */
void            sd_pulse_set_default     (SdPulse    *pulse,
                                          SdSide      side,
                                          const char *device);
/* The device's active port, by the SdPort's name. */
void            sd_pulse_set_port        (SdPulse    *pulse,
                                          SdSide      side,
                                          const char *device,
                                          const char *port);

/* The Default input's peak since the last call, from 0 to 1. */
typedef void (*SdPulseLevel) (double level, gpointer data);

/* Calls level about 25 times a second with what the Default input hears, following the
   default as it changes, until it's called with NULL. Nothing is opened on the server
   meanwhile otherwise: the popup asks only while it's shown. */
void            sd_pulse_watch_input_level (SdPulse     *pulse,
                                            SdPulseLevel level,
                                            gpointer     data);
/* The profile of the card at an SdCard's index, by the SdProfile's name. */
void            sd_pulse_set_profile     (SdPulse    *pulse,
                                          guint32     card,
                                          const char *profile);
void            sd_pulse_free            (SdPulse *pulse);
