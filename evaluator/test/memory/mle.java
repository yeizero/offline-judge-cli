import java.util.ArrayList;
import java.util.Arrays;

public class mle {
    public static void main(String[] args) {
        System.out.println("Starting massive memory allocation...");

        ArrayList<int[]> bigData = new ArrayList<>();

        final int outerSize = 40_000_00;
        final int innerSize = 100;

        for (int i = 0; i < outerSize; i++) {
            int[] arr = new int[innerSize];
            Arrays.fill(arr, i);
            bigData.add(arr);
        }

        System.out.println("Allocation complete. Press Enter to exit...");
    }
}