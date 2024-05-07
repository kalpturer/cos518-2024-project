import matplotlib.pyplot as plt
import numpy as np

def plot_histogram(input_file):
    with open(input_file, 'r') as f:
        lines = f.readlines()

    data = []
    for line in lines[2:]:
        if line[0] == 'R':
            print("Warning: timedout requests in data")
        else:
            data.append(int(line.strip()[:-2]))

    # remove outliers
    print(np.percentile(data, 99))
    threshold = np.percentile(data, 99)
    data = np.array(data)
    filtered_data = data[data <= threshold]

    print("Median latency:", np.median(data))
    print("outliers within one percent:", data[data > threshold])


    plt.hist(filtered_data, bins=100, align='left', edgecolor='black')
    plt.xlabel('Latency (ms)')
    plt.ylabel('Frequency')
    plt.title('Latency Distribution 3 Replicas (within 99th percentile)')
    plt.grid(True)
    plt.show()

# Example usage:
#input_file = './results_3_replicas_virginia_tokyo_london/results_0.00conflict/exp3_london_sleep{25}_time{30}.txt'
#input_file = './results_3_replicas_virginia_tokyo_london/results_0.02conflict/exp3_london_sleep{25}_time{30}.txt'
input_file = './results_3_replicas_virginia_tokyo_london/results_1.00conflict/exp3_london_sleep{100}_time{30}.txt'


plot_histogram(input_file)
