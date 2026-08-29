public class Chain {
    public static void main(String[] args) {
        long n = Long.parseLong(args[0]);
        long sum = 0;
        for (long i = 1; i <= n; i++) sum = (sum + i) % 1000000007L;
        System.out.println(sum);
    }
}
