#include <QtQml/qqml.h>

#include "padproxy_gui/src/gui_bridge.cxxqt.h"

extern "C" void padproxy_register_qml_types()
{
    qmlRegisterType<PadProxyController>("com.benwbooth.padproxy", 1, 0, "PadProxyController");
}
