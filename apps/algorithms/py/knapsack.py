def knapsack(weights, values, cap):
    n = len(weights)
    dp = [[0 for c in range(cap + 1)] for i in range(n + 1)]
    for i in range(1, n + 1):
        for c in range(cap + 1):
            dp[i][c] = dp[i - 1][c]
            if weights[i - 1] <= c:
                take = values[i - 1] + dp[i - 1][c - weights[i - 1]]
                if take > dp[i][c]:
                    dp[i][c] = take
    return dp[n][cap]

print(f"0/1 knapsack (cap 7): {knapsack([1, 3, 4, 5], [1, 4, 5, 7], 7)}")
