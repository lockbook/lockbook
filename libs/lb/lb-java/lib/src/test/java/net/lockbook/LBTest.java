package net.lockbook;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import java.util.Random;

import org.junit.Test;

public class LBTest {

    @Test
    public void someLibraryMethodReturnsTrue() throws Err {
        Lb.init(testDir());

        Err exc = null;
        try {
            Lb.createAccount("invalid username", null, true);
        } catch (Err e) {
            exc = e;
        }
        assertEquals(EKind.UsernameInvalid, exc.kind);

        Account account = Lb.createAccount(random(), "http://127.0.0.1:8000", true);
        assertNotNull(account.uname);
        assertNotNull(account.apiUrl);
    }

    static String random() {
        int length = 4;
        String characters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        Random random = new Random();
        StringBuilder result = new StringBuilder();

        for (int i = 0; i < length; i++) {
            int index = random.nextInt(characters.length());
            result.append(characters.charAt(index));
        }

        return result.toString();
    }

    static String testDir() {
        return "/tmp/" + random();
    }

    static void assertNoErr(Err err) {
        if (err != null) {
            System.err.println("msg: " + err.msg);
            System.err.println("code: " + err.kind);
            System.err.println("trace: " + err.trace);

            // fail this test
            assertNull(err);
        }
    }
}
