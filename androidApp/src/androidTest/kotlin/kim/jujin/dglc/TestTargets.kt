package kim.jujin.dglc

/** A renderer input target in the core's JSON form; [name] keeps each one distinct. */
internal fun paneTarget(name: String) =
    """{"connection":"$name","pane_id":"$name","generation":{"connection":1,"pane":1}}"""
