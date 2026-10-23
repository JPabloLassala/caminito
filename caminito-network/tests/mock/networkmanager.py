"""python-dbusmock's networkmanager template, with NetworkManager's AgentManager.

The template has no AgentManager, so a secret agent has nothing to register with. Here it's
an object of the mock, added as the template loads, so it's there before NetworkManager's
name is: an agent registers the moment the name appears.

The template's own mock methods, as AddWiFiDevice, are found on the module that's loaded, so
they're imported here too.
"""

# SPDX-License-Identifier: GPL-2.0-or-later

from dbusmock.templates import networkmanager
from dbusmock.templates.networkmanager import *  # noqa: F403, the template's attributes and methods

AGENT_MANAGER_OBJ = "/org/freedesktop/NetworkManager/AgentManager"
AGENT_MANAGER_IFACE = "org.freedesktop.NetworkManager.AgentManager"


def load(mock, parameters):
    networkmanager.load(mock, parameters)
    mock.AddObject(
        AGENT_MANAGER_OBJ,
        AGENT_MANAGER_IFACE,
        {},
        [
            ("Register", "s", "", ""),
            ("RegisterWithCapabilities", "su", "", ""),
            ("Unregister", "", "", ""),
        ],
    )
