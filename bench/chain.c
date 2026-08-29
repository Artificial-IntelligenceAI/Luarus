#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv) {
    long long n = atoll(argv[1]);
    long long sum = 0;
    for (long long i = 1; i <= n; i++) sum = (sum + i) % 1000000007LL;
    printf("%lld\n", sum);
    return 0;
}
