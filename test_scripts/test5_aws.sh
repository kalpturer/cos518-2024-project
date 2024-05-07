#!/bin/bash

s=125
t=30
c=1.00

#domestic
ssh -i "~/aws/aws-california.pem" ec2-user@ec2-54-183-230-57.us-west-1.compute.amazonaws.com "./target/release/project -g 3.16.125.199:6000 -l 172.31.13.142:6001 -p 54.183.230.57:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_california_sleep{$s}_time{$t}.txt &
ssh -i "~/aws/aws-ohio.pem" ec2-user@ec2-3-16-125-199.us-east-2.compute.amazonaws.com "./target/release/project -g 54.147.189.19:6000 -l 172.31.41.11:6001 -p 3.16.125.199:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_ohio_sleep{$s}_time{$t}.txt &
ssh -i "~/aws/aws-virginia.pem" ec2-user@ec2-54-147-189-19.compute-1.amazonaws.com "./target/release/project -g 54.214.191.51:6000 -l 172.31.25.220:6001 -p 54.147.189.19:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_virginia_sleep{$s}_time{$t}.txt &
ssh -i "~/aws/aws-oregon.pem" ec2-user@ec2-54-214-191-51.us-west-2.compute.amazonaws.com "./target/release/project -g 3.96.160.85:6000 -l 172.31.21.227:6001 -p 54.214.191.51:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_oregon_sleep{$s}_time{$t}.txt &
ssh -i "~/aws/aws-canada.pem" ec2-user@ec2-3-96-160-85.ca-central-1.compute.amazonaws.com "./target/release/project -g 54.183.230.57:6000 -l 172.31.3.162:6001 -p 3.96.160.85:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_canada_sleep{$s}_time{$t}.txt &


#global
#ssh -i "~/aws/aws-key-virginia.pem" ec2-user@ec2-54-226-130-94.compute-1.amazonaws.com "./cos518-2024-project/target/release/project -g 18.181.216.224:6000 -l 172.31.25.206:6001 -p 54.226.130.94:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_virginia_sleep{$s}_time{$t}.txt &
#ssh -i "~/aws/aws-key-tokyo.pem" ec2-user@ec2-18-181-216-224.ap-northeast-1.compute.amazonaws.com "./cos518-2024-project/target/release/project -g 35.178.195.147:6000 -l 172.31.9.230:6001 -p 18.181.216.224:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_tokyo_sleep{$s}_time{$t}.txt &
#ssh -i "~/aws/aws-key-london.pem" ec2-user@ec2-35-178-195-147.eu-west-2.compute.amazonaws.com "./cos518-2024-project/target/release/project -g 3.107.26.193:6000 -l 172.31.43.213:6001 -p 35.178.195.147:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_london_sleep{$s}_time{$t}.txt &
#ssh -i "~/aws/aws-key-sydney.pem" ec2-user@ec2-3-107-26-193.ap-southeast-2.compute.amazonaws.com "./cos518-2024-project/target/release/project -g 18.228.191.253:6000 -l 172.31.8.117:6001 -p 3.107.26.193:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_sydney_sleep{$s}_time{$t}.txt &
#ssh -i "~/aws/aws-key-brazil.pem" ec2-user@ec2-18-228-191-253.sa-east-1.compute.amazonaws.com "./cos518-2024-project/target/release/project -g 54.226.130.94:6000 -l 172.31.9.77:6001 -p 18.228.191.253:6001 --time-sleep $s --experiment-time $t --rate $c" > results/exp3_brazil_sleep{$s}_time{$t}.txt &
