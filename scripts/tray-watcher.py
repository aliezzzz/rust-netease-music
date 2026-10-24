#!/usr/bin/env python3
"""Minimal StatusNotifier host for protocol tests under an isolated D-Bus session."""
import json
from pathlib import Path
import sys
from gi.repository import Gio, GLib

output = Path(sys.argv[1])
xml = '''<node><interface name="org.kde.StatusNotifierWatcher">
<method name="RegisterStatusNotifierItem"><arg type="s" direction="in"/></method>
<method name="RegisterStatusNotifierHost"><arg type="s" direction="in"/></method>
<property name="RegisteredStatusNotifierItems" type="as" access="read"/>
<property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
<property name="ProtocolVersion" type="i" access="read"/>
<signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
</interface></node>'''
info = Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0]
items = []

def method(connection, sender, path, interface, method, params, invocation):
    if method == 'RegisterStatusNotifierItem':
        value = params.unpack()[0]
        service, item_path = (sender, value) if value.startswith('/') else (value, '/StatusNotifierItem')
        items.append(service + item_path)
        output.write_text(json.dumps({'service': service, 'path': item_path}))
        connection.emit_signal(None, path, interface, 'StatusNotifierItemRegistered', GLib.Variant('(s)', (service + item_path,)))
    invocation.return_value(GLib.Variant('()', ()))

def prop(connection, sender, path, interface, name):
    return {'RegisteredStatusNotifierItems': GLib.Variant('as', items), 'IsStatusNotifierHostRegistered': GLib.Variant('b', True), 'ProtocolVersion': GLib.Variant('i', 0)}[name]

connection = Gio.bus_get_sync(Gio.BusType.SESSION)
connection.register_object('/StatusNotifierWatcher', info, method, prop, None)
owner = Gio.bus_own_name_on_connection(connection, 'org.kde.StatusNotifierWatcher', Gio.BusNameOwnerFlags.NONE, None, None)
GLib.MainLoop().run()
