#!/bin/bash

s=20
t=30
c=0.0

ssh -i "~/aws/aws-key-virginia.pem" ec2-user@ec2-54-226-130-94.compute-1.amazonaws.com "./cos518-2024-project/target/release/project -g 18.181.216.224:6000 -l 172.31.25.206:6001 -p 54.226.130.94:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_virginia_sleep{$s}_time{$t}.txt &
ssh -i "~/aws/aws-key-tokyo.pem" ec2-user@ec2-18-181-216-224.ap-northeast-1.compute.amazonaws.com "./cos518-2024-project/target/release/project -g 35.178.195.147:6000 -l 172.31.9.230:6001 -p 18.181.216.224:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_tokyo_sleep{$s}_time{$t}.txt &
ssh -i "~/aws/aws-key-london.pem" ec2-user@ec2-35-178-195-147.eu-west-2.compute.amazonaws.com "./cos518-2024-project/target/release/project -g 3.107.26.193:6000 -l 172.31.43.213:6001 -p 35.178.195.147:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_london_sleep{$s}_time{$t}.txt &
ssh -i "~/aws/aws-key-sydney.pem" ec2-user@ec2-3-107-26-193.ap-southeast-2.compute.amazonaws.com "./cos518-2024-project/target/release/project -g 18.228.191.253:6000 -l 172.31.8.117:6001 -p 3.107.26.193:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_sydney_sleep{$s}_time{$t}.txt &
ssh -i "~/aws/aws-key-brazil.pem" ec2-user@ec2-18-228-191-253.sa-east-1.compute.amazonaws.com "./cos518-2024-project/target/release/project -g 54.226.130.94:6000 -l 172.31.9.77:6001 -p 18.228.191.253:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_brazil_sleep{$s}_time{$t}.txt &
