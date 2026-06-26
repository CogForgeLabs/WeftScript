def quicksort(a):
    if len(a) <= 1:
        return a
    pivot = a[0]
    rest = a[1:len(a)]
    less = [x for x in rest if x < pivot]
    more = [x for x in rest if x >= pivot]
    return quicksort(less) + [pivot] + quicksort(more)

print(quicksort([5, 2, 9, 1, 5, 6, 3, 8, 0, 7]))
