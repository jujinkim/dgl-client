package kim.jujin.dglc

import kotlin.test.*

class TerminalLinkTest {
    @Test fun onlyBoundedHttpLinksWithAnAuthorityOpen() {
        for (link in listOf("https://example.com/a?q=(1)#x", "http://example.com:8080/한글", "https://user@example.com/",
            "http://[::1]:8080/", "https://my_host.example.com/path"))
            assertEquals(link, terminalWebLink(link), link)
        assertEquals("https://EXAMPLE.com/Path", terminalWebLink("HTTPS://EXAMPLE.com/Path"), "browser filters match a lowercase scheme")
        val longest = "https://example.com/" + "a".repeat(MAX_TERMINAL_LINK - 20)
        assertEquals(longest, terminalWebLink(longest))
        assertNull(terminalWebLink(longest + "a"))
        for (rejected in listOf(null, "", "javascript:alert(1)", "intent://example.com#Intent;scheme=https;end", "file:///etc/passwd",
            "content://media/external", "data:text/html,x", "ftp://example.com/", "https://", "https:///path", "//example.com",
            "www.example.com", "https://exa mple.com", "https://example.com/\n", "https://example.com/\u0007",
            "https://example.com/\u0085", "https://example.com/%zz"))
            assertNull(terminalWebLink(rejected), rejected)
    }

    @Test fun copyWithoutLineBreaksJoinsRowsAndDropsTheirIndentation() {
        assertEquals("https://example.com/aaaabbbb/end", terminalTextWithoutLineBreaks("https://example.com/aaaa\n  bbbb/end"))
        assertEquals("abc", terminalTextWithoutLineBreaks("a \r\n\tb\r\nc"))
        assertEquals("a b", terminalTextWithoutLineBreaks("a b"), "spaces inside a row stay")
        assertEquals("", terminalTextWithoutLineBreaks("\n\n"))
    }
}
