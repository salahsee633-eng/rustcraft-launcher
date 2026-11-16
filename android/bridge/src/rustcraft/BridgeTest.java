package rustcraft;

import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;

/**
 * M3.1 probe: writes proof that the bundled OpenJDK really started.
 *
 * <p>Invoked through JLI_Launch with one argument: the file to write. Keeping
 * the result in a file avoids relying on stdout, which Android discards for
 * apps.
 */
public final class BridgeTest {
    private BridgeTest() {}

    public static void main(String[] args) throws Exception {
        String path = args.length > 0 ? args[0] : "jvm-result.txt";
        String result =
                "java.version=" + System.getProperty("java.version")
                        + "\njava.vm.name=" + System.getProperty("java.vm.name")
                        + "\njava.home=" + System.getProperty("java.home")
                        + "\nos.arch=" + System.getProperty("os.arch") + "\n";

        Writer writer = new OutputStreamWriter(new FileOutputStream(path), "UTF-8");
        try {
            writer.write(result);
        } finally {
            writer.close();
        }
    }
}
