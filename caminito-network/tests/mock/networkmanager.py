"""python-dbusmock's networkmanager template, with NetworkManager's AgentManager.

The template has no AgentManager, so a secret agent has nothing to register with. Here it's
an object of the mock, added as the template loads, so it's there before NetworkManager's
name is: an agent registers the moment the name appears.
"""

# SPDX-License-Identifier: GPL-2.0-or-later

from dbusmock.templates import networkmanager
from dbusmock.templates.networkmanager import (  # noqa: F401, the template's module attributes
    BUS_NAME,
    IS_OBJECT_MANAGER,
    MAIN_OBJ,
    SYSTEM_BUS,
)

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
